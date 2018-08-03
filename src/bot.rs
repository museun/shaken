use crate::color::RGB;
use crate::config::Config;
use crate::conn::Conn;
use crate::message::{Envelope, Message};

use crossbeam_channel as channel;
use parking_lot::{Mutex, RwLock};
use scoped_threadpool::Pool;

use std::collections::VecDeque;
use std::time;

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

struct Handlers(RwLock<Vec<Handler>>);

#[derive(Clone, PartialEq, Debug)]
pub struct User {
    pub display: String,
    pub color: RGB,
    pub userid: String,
}

struct Inspected {
    output: RwLock<VecDeque<String>>,
    inspect: Mutex<Box<Fn(&User, &str) + Send + Sync + 'static>>, // only 1 write
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

pub(crate) struct Inner {
    pub(crate) user: User,
    pub(crate) owners: Vec<String>,
}

pub struct Bot {
    pub(crate) inner: Mutex<Inner>,
    pub(crate) channel: String,
    conn: Conn,
    handlers: Handlers,
    inspected: Inspected,
}

impl Bot {
    pub fn new<C>(conn: C, config: &Config) -> Self
    where
        C: Into<Conn>,
    {
        let conn = conn.into();

        let inner = Mutex::new(Inner {
            user: User {
                display: config.twitch.name.to_string(),
                color: RGB::from("fc0fc0"),
                userid: "".into(), // we don't have our id yet
            },
            owners: config.twitch.owners.clone(),
        });

        let inspected = Inspected {
            output: RwLock::new(VecDeque::new()),
            inspect: Mutex::new(Box::new(|_, _| {})),
        };

        Self {
            inner,
            channel: config.twitch.channel.to_string(),
            inspected,
            conn,
            handlers: Handlers(RwLock::new(vec![])),
        }
    }

    pub fn run(&self) {
        self.register();
        let interval = time::Duration::from_secs(1);
        use crossbeam_channel::tick;

        // maybe use a bounded channel
        let (tx, rx) = channel::unbounded();

        // so the dispatch handling is easier, just keep sending the same empty message for non-message events
        // TODO make this into an enum, commands need to be an enum anyway
        let empty = Message::default();

        // XXX: only need 2 threads for now
        let mut pool = Pool::new(2);
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
                    recv(tick(interval), _) => {
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
                            //msg.tags.get("color")
                            color: RGB::from("fc0fc0"), // TODO get this from the config
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
                        // TODO parse these recursively
                        env.data = env.data[s.len()..].trim().to_string();
                        f(&self, &env);
                    }
                }
                (Some(ref env), Handler::Passive(ref f), _) => {
                    debug!("running passive");
                    f(&self, &env);
                }
                (None, Handler::Raw(cmd, ref f), _) => {
                    if cmd == &msg.command {
                        debug!("running raw handler: {}", cmd);
                        f(&self, &msg);
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
        inner.user.clone() // TODO: why is this a clone?
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
        self.send(&format!("JOIN {}", ch))
    }

    pub fn send(&self, data: &str) {
        self.conn.write(data)
    }

    pub fn set_inspect<F>(&self, f: F)
    where
        F: Fn(&User, &str) + Send + Sync + 'static,
    {
        *self.inspected.inspect.lock() = Box::new(f);
    }

    pub fn get_commands(&self) -> Vec<String> {
        let mut vec = vec![]; // TODO pre-allocate this
        for hn in self.handlers.0.read().iter() {
            if let Handler::Command(cmd, _) = hn {
                vec.push(cmd.to_string());
            }
        }
        vec
    }

    fn privmsg(&self, ch: &str, msg: &str) {
        self.send(&format!("PRIVMSG {} :{}", ch, msg));
        self.inspected.write(&msg);
    }

    fn register(&self) {
        trace!("registering");

        // ircv3 stuff
        self.conn.write("CAP REQ :twitch.tv/tags");
        self.conn.write("CAP REQ :twitch.tv/membership");
        self.conn.write("CAP REQ :twitch.tv/commands");

        self.conn
            .write(&format!("PASS {}", env!("SHAKEN_TWITCH_PASSWORD")));

        let inner = self.inner.lock();
        self.conn.write(&format!("NICK {}", inner.user.display));

        // this would be needed for a real irc server
        // self.conn
        //     .write(&format!("USER {} * 8 :{}", "shaken_bot", "shaken_bot"));

        trace!("registered");
    }

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
