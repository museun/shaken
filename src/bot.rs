use crate::color::Color;
use crate::config::Config;
use crate::conn::Conn;
use crate::message::{Envelope, Message};

use crossbeam_channel as channel;

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, RwLock};
use std::time;

type Inspector = fn(&User, &str);

pub struct Bot {
    pub(crate) inner: RwLock<Inner>,
    handlers: Handlers,
    inspected: Inspected,
}

pub enum Handler {
    Command(
        &'static str,
        RwLock<Box<Fn(&Bot, &Envelope) + Send + Sync + 'static>>,
    ),
    Passive(RwLock<Box<Fn(&Bot, &Envelope) + Send + Sync + 'static>>),
    Raw(
        &'static str,
        RwLock<Box<Fn(&Bot, &Message) + Send + Sync + 'static>>,
    ),
    Tick(RwLock<Box<Fn(&Bot) + Send + Sync + 'static>>),
}

struct Handlers(RwLock<Vec<Handler>>);

pub(crate) struct Inner {
    pub(crate) conn: Arc<Conn>,
    user: User,
    owners: Vec<String>,
}

#[derive(Clone)]
pub struct User {
    pub display: String,
    pub color: Color,
    pub userid: String,
}

struct Inspected {
    output: RwLock<VecDeque<String>>,
    inspect: RwLock<Inspector>,
}

impl Inspected {
    pub fn inspect(&self, me: &User) {
        let inspect = self.inspect.read().unwrap();
        let mut list = self.output.write().unwrap();
        for el in list.drain(..) {
            inspect(&me, &el);
        }
    }

    pub fn write(&self, data: &str) {
        self.output.write().unwrap().push_back(data.into())
    }
}

impl Bot {
    pub fn new(conn: Conn, config: &Config) -> Self {
        let inner = RwLock::new(Inner {
            conn: Arc::new(conn),
            user: User {
                display: "".into(), // we don't have our name yet
                color: Color::from(None),
                userid: "".into(), // we don't have our id yet
            },
            owners: config.twitch.owners.clone(),
        });

        let inspected = Inspected {
            output: RwLock::new(VecDeque::new()),
            inspect: RwLock::new(|_, _| {}),
        };

        Self {
            inner,
            inspected,
            handlers: Handlers(RwLock::new(vec![])),
        }
    }

    pub fn run(&self, config: &Config) {
        self.register(&config);

        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(4)
            .start_handler(|n| trace!("starting thread: {}", n))
            .exit_handler(|n| trace!("exiting thread: {}", n))
            .build()
            .unwrap();

        let tick = channel::tick(time::Duration::from_secs(1));
        let (tx, rx) = channel::unbounded(); // maybe use a bounded channel
        {
            let tx = tx.clone();
            pool.install(|| {
                let mut caps = HashMap::new();
                'main: loop {
                    let msg = {
                        let inner = self.inner.read().unwrap();
                        match inner.conn.read() {
                            Some(line) => Message::parse(&line),
                            None => break 'main,
                        }
                    };

                    if msg.command == "GLOBALUSERSTATE" {
                        trace!("got our caps");
                        caps = msg.tags.clone();

                        let color = caps.get("color");
                        let user = User {
                            display: caps["display-name"].to_string(),
                            color: Color::from(color),
                            userid: caps["user-id"].to_string(),
                        };

                        let mut inner = self.inner.write().unwrap();
                        inner.user = user;
                    }

                    tx.send(msg);
                }
            });
        }

        // so the dispatch handling is easier, just keep sending the same empty message for non-message events
        // TODO make this into an enum, commands need to be an enum anyway
        let empty = Message::default();

        'select: loop {
            select!{
                recv(rx, msg) => {
                    match msg {
                        Some(msg) => {
                            self.dispatch(&msg, false);
                            self.inspected.inspect(&self.user_info());
                        }
                        None => break 'select
                    }
                },
                recv(tick, _) => {
                    self.dispatch(&empty, true)
                },
            }
        }
    }

    pub(crate) fn dispatch(&self, msg: &Message, tick: bool) {
        let env = if msg.command == "PRIVMSG" {
            Some(Envelope::from_msg(&msg))
        } else {
            None
        };

        for hn in self.handlers.0.read().unwrap().iter() {
            match (&env, hn, tick) {
                (Some(ref env), Handler::Command(s, ref f), _) => {
                    if env.data.starts_with(s) {
                        debug!("running command: {}", s);
                        // make a clone because we're mutating it
                        let mut env = env.clone();
                        // trim the command
                        env.data = env.data[s.len()..].trim().to_string();
                        f.read().unwrap()(&self, &env)
                    }
                }
                (Some(ref env), Handler::Passive(ref f), _) => {
                    f.read().unwrap()(&self, &env);
                }
                (None, Handler::Raw(cmd, ref f), _) => {
                    if cmd == &msg.command {
                        debug!("running raw handler: {}", cmd);
                        f.read().unwrap()(&self, &msg)
                    }
                }
                (_, Handler::Tick(ref f), true) => {
                    f.read().unwrap()(&self); // maybe send the time delta
                }
                _ => {}
            }
        }
    }

    pub fn is_owner_id(&self, id: &str) -> bool {
        let inner = self.inner.read().unwrap();
        inner.owners.contains(&id.to_string()) // why
    }

    pub fn user_info(&self) -> User {
        let inner = self.inner.read().unwrap();
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
        *self.inspected.inspect.write().unwrap() = f;
    }

    pub fn get_commands(&self) -> Vec<String> {
        let mut vec = vec![];
        for hn in self.handlers.0.read().unwrap().iter() {
            if let Handler::Command(cmd, _) = hn {
                vec.push(cmd.to_string());
            }
        }
        vec
    }

    fn conn(&self) -> Arc<Conn> {
        let inner = self.inner.read().unwrap();
        Arc::clone(&inner.conn)
    }

    fn privmsg(&self, ch: &str, msg: &str) {
        self.send(&format!("PRIVMSG {} :{}", ch, msg));
        self.inspected.write(&msg);
    }

    #[cfg(not(test))]
    fn register(&self, config: &Config) {
        let proto = self.conn();
        // ircv3 stuff
        proto.write("CAP REQ :twitch.tv/tags");
        proto.write("CAP REQ :twitch.tv/membership");
        proto.write("CAP REQ :twitch.tv/commands");

        proto.write(&format!("PASS {}", &config.twitch.pass));

        // TODO: determine if we'll be connecting to actual IRC server.
        //  this is needed for real irc servers
        // proto.write(&format!("NICK {}", &config.twitch.nick));
        // proto.write(&format!(
        //     "USER {} * 8 :{}",
        //     &config.twitch.nick, &config.twitch.nick
        // ));
    }

    #[cfg(test)]
    fn register(&self, _config: &Config) {}

    pub fn on_command<F>(&self, cmd: &'static str, f: F)
    where
        F: Fn(&Bot, &Envelope) + Send + Sync + 'static,
    {
        self.add_handler(Handler::Command(cmd, RwLock::new(Box::new(f))));
    }

    pub fn on_passive<F>(&self, f: F)
    where
        F: Fn(&Bot, &Envelope) + Send + Sync + 'static,
    {
        self.add_handler(Handler::Passive(RwLock::new(Box::new(f))));
    }

    pub fn on_raw<F>(&self, cmd: &'static str, f: F)
    where
        F: Fn(&Bot, &Message) + Send + Sync + 'static,
    {
        self.add_handler(Handler::Raw(cmd, RwLock::new(Box::new(f))));
    }

    pub fn on_tick<F>(&self, f: F)
    where
        F: Fn(&Bot) + Send + Sync + 'static,
    {
        self.add_handler(Handler::Tick(RwLock::new(Box::new(f))));
    }

    fn add_handler(&self, hn: Handler) {
        self.handlers.0.write().unwrap().push(hn)
    }
}
