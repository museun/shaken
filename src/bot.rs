use crate::config::Config;
use crate::conn::Proto;
use crate::message::{Envelope, Message};

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, RwLock};

type Inspector = fn(&HashMap<String, String>, &str, &str);

pub struct Bot {
    inner: RwLock<Inner<'static>>,
    handlers: RwLock<Vec<Handler>>,
    output: RwLock<VecDeque<String>>,
    inspect: RwLock<Inspector>,
    nick: RwLock<String>,
}

struct Inner<'a> {
    proto: Arc<Box<Proto + 'a>>,
    #[allow(dead_code)] // this isn't used yet
    channels: Vec<String>,
}

impl Bot {
    pub fn new(proto: impl Proto + 'static + Send + Sync, config: &Config) -> Self {
        let inner = RwLock::new(Inner {
            proto: Arc::new(Box::new(proto)),
            channels: config.twitch.channels.to_vec(),
        });

        Self {
            inner,
            handlers: RwLock::new(vec![]),
            inspect: RwLock::new(|_, _, _| {}),
            output: RwLock::new(VecDeque::new()),
            nick: RwLock::new(config.twitch.nick.to_string()),
        }
    }

    pub fn proto(&'b self) -> Arc<Box<Proto + 'a>> {
        let inner = self.inner.read().unwrap();
        Arc::clone(&inner.proto)
    }

    pub fn nick(&self) -> String {
        let inner = self.nick.read().unwrap();
        inner.to_string()
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

    pub fn set_inspect(&self, f: fn(&HashMap<String, String>, &str, &str)) {
        *self.inspect.write().unwrap() = f;
    }

    fn privmsg(&self, ch: &str, msg: &str) {
        self.proto().privmsg(ch, msg);
        self.output.write().unwrap().push_back(msg.into());
    }

    fn register(&self, config: &Config) {
        let proto = self.proto();
        // ircv3 stuff
        proto.send("CAP REQ :twitch.tv/tags");
        proto.send("CAP REQ :twitch.tv/membership");
        proto.send("CAP REQ :twitch.tv/commands");

        proto.send(&format!("PASS {}", &config.twitch.pass));
        proto.send(&format!("NICK {}", &config.twitch.nick));
        // this is needed for real irc servers
        proto.send(&format!(
            "USER {} * 8 :{}",
            &config.twitch.nick, &config.twitch.nick
        ));
    }

    pub fn run(&self, config: &Config) {
        self.register(&config);

        let mut caps = HashMap::new();
        // can't use the write lock from this point on
        while let Some(line) = self.proto().read() {
            let msg = Message::parse(&line);
            // TODO determine if we've actually gotten the right cap response

            if msg.command == "GLOBALUSERSTATE" {
                debug!("got our caps");
                caps = msg.tags.clone();
                //display-name=shaken_bot
                *self.nick.write().unwrap() = caps["display-name"].to_string();
            }

            // hide the ping spam
            if msg.command != "PING" {
                debug!("{}", msg);
            }

            let env = if msg.command == "PRIVMSG" {
                Some(Envelope::from_msg(&msg))
            } else {
                None
            };

            // TODO run this on a threadpool
            let handlers = self.handlers.read().unwrap();
            for hn in handlers.iter() {
                match (&env, hn) {
                    (Some(ref env), Handler::Command(s, f)) => {
                        if env.data.starts_with(s) {
                            debug!("running command: {}", s);
                            // make a clone because we're mutating it
                            let mut env = env.clone();
                            // trim the command
                            env.data = env.data[s.len()..].trim().to_string();
                            f(&self, &env)
                        }
                    }
                    (Some(ref env), Handler::Passive(f)) => f(&self, &env),
                    (None, Handler::Raw(cmd, f)) => {
                        if cmd == &msg.command {
                            // hide the ping spam
                            if &msg.command != "PING" {
                                debug!("running raw handler: {}", cmd);
                            }
                            f(&self, &msg)
                        }
                    }
                    _ => {}
                }
            }

            let me = self.nick();
            let inspect = self.inspect.read().unwrap();
            let mut list = self.output.write().unwrap();
            for el in list.drain(..) {
                inspect(&caps, &me, &el);
            }
        }
    }

    pub fn on_command<F>(&self, cmd: &'static str, f: F)
    where
        F: Fn(&Bot, &Envelope) + 'static,
    {
        self.handlers
            .write()
            .unwrap()
            .push(Handler::Command(cmd, Box::new(f)));
    }

    pub fn on_passive<F>(&self, f: F)
    where
        F: Fn(&Bot, &Envelope) + 'static,
    {
        self.handlers
            .write()
            .unwrap()
            .push(Handler::Passive(Box::new(f)));
    }

    pub fn on_raw<F>(&self, cmd: &'static str, f: F)
    where
        F: Fn(&Bot, &Message) + 'static,
    {
        self.handlers
            .write()
            .unwrap()
            .push(Handler::Raw(cmd, Box::new(f)));
    }
}

pub enum Handler {
    Command(&'static str, Box<Fn(&Bot, &Envelope)>),
    Passive(Box<Fn(&Bot, &Envelope)>),
    Raw(&'static str, Box<Fn(&Bot, &Message)>),
}
