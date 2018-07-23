use crate::config::Config;
use crate::conn::Proto;
use crate::message::{Envelope, Message};

use std::rc::Rc;
use std::sync::RwLock;

pub struct Bot {
    inner: RwLock<Inner<'static>>,
    handlers: RwLock<Vec<Handler>>,
}

struct Inner<'a> {
    proto: Rc<Box<Proto + 'a>>,
    channels: Vec<String>,
    nick: String,
}

impl Bot {
    pub fn new(proto: impl Proto + 'static, config: &Config) -> Self {
        let inner = RwLock::new(Inner {
            proto: Rc::new(Box::new(proto)),
            channels: config.twitch.channels.to_vec(),
            nick: config.twitch.nick.to_string(),
        });

        Self {
            inner,
            handlers: RwLock::new(vec![]),
        }
    }

    pub fn proto(&'b self) -> Rc<Box<Proto + 'a>> {
        let inner = self.inner.read().unwrap();
        Rc::clone(&inner.proto)
    }

    pub fn nick(&self) -> String {
        let inner = self.inner.read().unwrap();
        inner.nick.to_string()
    }

    pub fn run(&self, config: &Config) {
        self.proto().send(&format!("PASS {}", &config.twitch.pass));
        self.proto().send(&format!("NICK {}", &config.twitch.nick));
        // this is needed for real irc servers
        self.proto().send(&format!(
            "USER {} * 8 :{}",
            &config.twitch.nick, &config.twitch.nick
        ));

        // can't use the write lock from this point on
        while let Some(line) = self.proto().read() {
            let msg = Message::parse(&line);
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
                            env.data = env.data[s.len()..].to_string();
                            f(&self, &env)
                        }
                    }
                    (Some(ref env), Handler::Passive(f)) => {
                        f(&self, &env);
                    }
                    (None, Handler::Raw(cmd, f)) => {
                        if cmd == &msg.command {
                            // hide the ping spam
                            if &msg.command != "PING" {
                                debug!("running server: {}", cmd);
                            }
                            f(&self, &msg)
                        }
                    }
                    _ => {}
                }
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
