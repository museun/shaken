use crate::color::Color;
use crate::config::Config;
use crate::conn::Conn;
use crate::message::{Envelope, Message};

use crossbeam_channel as channel;
use parking_lot as sync;
use scoped_threadpool::Pool;

use std::collections::VecDeque;
use std::sync::Arc;
use std::time;

type Inspector = fn(&User, &str);

// LOCK: determine whether we actually need to wrap these in a mutex
pub enum Handler {
    Command(
        &'static str,
        Box<Fn(&Bot, &Envelope) + Send + Sync + 'static>,
    ),
    Passive(Box<Fn(&Bot, &Envelope) + Send + Sync + 'static>),
    Raw(
        &'static str,
        Box<Fn(&Bot, &Message) + Send + Sync + 'static>,
    ),
    Tick(Box<Fn(&Bot) + Send + Sync + 'static>),
}

struct Handlers(sync::RwLock<Vec<Handler>>);

pub struct Bot {
    pub(crate) inner: sync::Mutex<Inner>,
    pub(crate) conn: Arc<Conn>,
    handlers: Handlers,
    inspected: Inspected,
}

pub(crate) struct Inner {
    pub(crate) user: User,
    pub(crate) owners: Vec<String>,
}

#[derive(Clone)]
pub struct User {
    pub display: String,
    pub color: Color,
    pub userid: String,
}

struct Inspected {
    output: sync::RwLock<VecDeque<String>>,
    inspect: sync::Mutex<Inspector>, // only 1 write
}

impl Inspected {
    pub fn inspect(&self, me: &User) {
        let inspect = self.inspect.lock();
        let mut list = self.output.write();
        for el in list.drain(..) {
            inspect(&me, &el);
        }
    }

    pub fn write(&self, data: &str) {
        self.output.write().push_back(data.into())
    }
}

impl Bot {
    pub fn new(conn: Conn, config: &Config) -> Self {
        let inner = sync::Mutex::new(Inner {
            user: User {
                display: "".into(), // we don't have our name yet
                color: Color::from("fc0fc0"),
                userid: "".into(), // we don't have our id yet
            },
            owners: config.twitch.owners.clone(),
        });

        let inspected = Inspected {
            output: sync::RwLock::new(VecDeque::new()),
            inspect: sync::Mutex::new(|_, _| {}),
        };

        Self {
            inner,
            inspected,
            conn: Arc::new(conn),
            handlers: Handlers(sync::RwLock::new(vec![])),
        }
    }

    pub fn run(&self, config: &Config) {
        self.register(&config);

        let tick = channel::tick(time::Duration::from_secs(1));
        let (tx, rx) = channel::unbounded(); // maybe use a bounded channel

        // so the dispatch handling is easier, just keep sending the same empty message for non-message events
        // TODO make this into an enum, commands need to be an enum anyway
        let empty = Message::default();

        let mut pool = Pool::new(4);
        pool.scoped(|scope| {
            scope.execute(move || 'events: loop {
                select!{
                    recv(rx, msg) => {
                        match msg {
                            Some(msg) => {
                                self.dispatch(&msg, false);
                                self.inspected.inspect(&self.user_info());
                            }
                            None => {
                                debug!("didn't get a message");
                                // this probably needs to kill the thread pool
                                break 'events;
                            }
                        }
                    },
                    recv(tick, _) => {
                        self.dispatch(&empty, true);
                    },
                }
            });

            scope.execute(move || {
                trace!("starting main loop");
                'main: loop {
                    let msg = {
                        match self.conn.read() {
                            Some(line) => Message::parse(&line),
                            None => break 'main,
                        }
                    };

                    if msg.command == "GLOBALUSERSTATE" {
                        trace!("got our caps");
                        let user = User {
                            display: msg.tags["display-name"].to_string(),
                            color: Color::from("fc0fc0"), //msg.tags.get("color")
                            userid: msg.tags["user-id"].to_string(),
                        };

                        let mut inner = self.inner.lock();
                        inner.user = user;
                    }

                    tx.send(msg);
                }
            });
        });
    }

    pub(crate) fn dispatch(&self, msg: &Message, tick: bool) {
        let env = if msg.command == "PRIVMSG" {
            Some(Envelope::from_msg(&msg))
        } else {
            None
        };

        for hn in self.handlers.0.read().iter() {
            match (&env, hn, tick) {
                (Some(ref env), Handler::Command(s, ref f), _) => {
                    if env.data.starts_with(s) {
                        debug!("running command: {}", s);
                        // make a clone because we're mutating it
                        let mut env = env.clone();
                        // trim the command
                        env.data = env.data[s.len()..].trim().to_string();
                        f(&self, &env)
                    }
                }
                (Some(ref env), Handler::Passive(ref f), _) => {
                    debug!("running passive");
                    f(&self, &env);
                }
                (None, Handler::Raw(cmd, ref f), _) => {
                    if cmd == &msg.command {
                        debug!("running raw handler: {}", cmd);
                        f(&self, &msg)
                    }
                }
                (_, Handler::Tick(ref f), true) => {
                    f(&self); // maybe send the time delta
                }
                _ => {}
            }
        }
    }

    pub fn is_owner_id(&self, id: &str) -> bool {
        let inner = self.inner.lock();
        inner.owners.contains(&id.to_string()) // why
    }

    pub fn user_info(&self) -> User {
        let inner = self.inner.lock();
        inner.user.clone()
    }

    pub fn reply(&self, env: &Envelope, msg: &str) {
        if msg.is_empty() {
            warn!("tried to reply with an empty message");
            return;
        }

        if let Some(who) = env.get_nick() {
            self.privmsg(&env.channel, &format!("@{}: {}", who, msg));
        } else {
            warn!("cannot reply with no nick");
        }
    }

    pub fn say(&self, env: &Envelope, msg: &str) {
        if msg.is_empty() {
            warn!("tried to reply with an empty message");
            return;
        }
        self.privmsg(&env.channel, msg);
    }

    pub fn join(&self, ch: &str) {
        self.conn().write(&format!("JOIN {}", ch))
    }

    pub fn send(&self, data: &str) {
        self.conn().write(data)
    }

    pub fn set_inspect(&self, f: Inspector) {
        *self.inspected.inspect.lock() = f;
    }

    pub fn get_commands(&self) -> Vec<String> {
        let mut vec = vec![];
        for hn in self.handlers.0.read().iter() {
            if let Handler::Command(cmd, _) = hn {
                vec.push(cmd.to_string());
            }
        }
        vec
    }

    fn conn(&self) -> Arc<Conn> {
        Arc::clone(&self.conn)
    }

    fn privmsg(&self, ch: &str, msg: &str) {
        self.send(&format!("PRIVMSG {} :{}", ch, msg));
        self.inspected.write(&msg);
    }

    #[cfg(not(test))]
    fn register(&self, config: &Config) {
        trace!("registering");

        // ircv3 stuff
        self.conn.write("CAP REQ :twitch.tv/tags");
        self.conn.write("CAP REQ :twitch.tv/membership");
        self.conn.write("CAP REQ :twitch.tv/commands");

        self.conn.write(&format!("PASS {}", &config.twitch.pass));

        // maybe this is needed
        self.conn.write(&format!("NICK {}", "shaken_bot"));
        self.conn
            .write(&format!("USER {} * 8 :{}", "shaken_bot", "shaken_bot"));

        trace!("registered");
    }

    #[cfg(test)]
    fn register(&self, _config: &Config) {}

    pub fn on_command<F>(&self, cmd: &'static str, f: F)
    where
        F: Fn(&Bot, &Envelope) + Send + Sync + 'static,
    {
        self.add_handler(Handler::Command(cmd, Box::new(f)));
    }

    pub fn on_passive<F>(&self, f: F)
    where
        F: Fn(&Bot, &Envelope) + Send + Sync + 'static,
    {
        self.add_handler(Handler::Passive(Box::new(f)));
    }

    pub fn on_raw<F>(&self, cmd: &'static str, f: F)
    where
        F: Fn(&Bot, &Message) + Send + Sync + 'static,
    {
        self.add_handler(Handler::Raw(cmd, Box::new(f)));
    }

    pub fn on_tick<F>(&self, f: F)
    where
        F: Fn(&Bot) + Send + Sync + 'static,
    {
        self.add_handler(Handler::Tick(Box::new(f)));
    }

    fn add_handler(&self, hn: Handler) {
        self.handlers.0.write().push(hn)
    }
}
