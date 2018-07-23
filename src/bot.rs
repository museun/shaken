use crate::config::Config;
use crate::conn::Proto;
use crate::message::{Envelope, Handler, Message};
use crate::state::State;

use std::sync::Mutex;
use std::time;

use rand::prelude::*;

pub struct Bot<'a> {
    proto: Box<Proto + 'a>,
    state: Mutex<State>,
    channels: Vec<String>,
    nick: &'a str,
}

impl<'a> Bot<'a> {
    pub fn new(proto: impl Proto + 'a, config: &'a Config) -> Self {
        Self {
            proto: Box::new(proto),
            state: Mutex::new(State::new(config.interval, config.chance)),
            channels: config.channels.to_vec(),
            nick: &config.nick,
        }
    }

    pub fn run(&self, config: &Config) {
        self.proto.send(&format!("PASS {}", &config.pass));
        self.proto.send(&format!("NICK {}", &config.nick));
        // this is needed for real irc servers
        self.proto
            .send(&format!("USER {} * 8 :{}", &config.nick, &config.nick));

        // TODO: move this out of this function
        let handlers = vec![
            Handler::Active("!speak", Bot::speak),
            Handler::Active("!version", Bot::version),
            Handler::Passive(Bot::check_mentions),
            Handler::Passive(Bot::auto_speak),
            Handler::Raw("PING", |bot, msg| {
                bot.proto.send(&format!("PONG :{}", &msg.data))
            }),
            Handler::Raw("001", |bot, _msg| {
                for ch in &bot.channels {
                    bot.proto.join(&ch)
                }
            }),
        ];

        while let Some(line) = self.proto.read() {
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

            for hn in &handlers {
                match (&env, hn) {
                    (Some(ref env), Handler::Active(s, f)) => {
                        if env.data.starts_with(s) {
                            debug!("running command: {}", s);
                            // make a clone because we're mutating it
                            let mut env = env.clone();
                            // trim the command
                            env.data = env.data[s.len()..].to_string();
                            f(&self, &env)
                        }
                    }
                    (Some(ref env), Handler::Passive(f)) => f(&self, &env),
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

    fn version(bot: &Bot, env: &Envelope) {
        // these are set by the build script
        let rev = option_env!("SHAKEN_GIT_REV").unwrap();
        let branch = option_env!("SHAKEN_GIT_BRANCH").unwrap();

        let msg = format!(
            "https://github.com/museun/shaken/commit/{} ('{}' branch)",
            rev, branch
        );

        bot.proto.privmsg(&env.channel, &msg)
    }
    fn speak(bot: &Bot, env: &Envelope) {
        trace!("trying to speak");
        if let Some(resp) = bot.state.lock().unwrap().generate() {
            trace!("speaking");
            bot.proto.privmsg(&env.channel, &resp)
        }
    }

    fn check_mentions(bot: &Bot, env: &Envelope) {
        let parts = env.data.split_whitespace();
        for part in parts {
            if part.starts_with('@') && &part[1..] == bot.nick {
                Bot::speak(&bot, &env);
                break;
            }
        }
    }

    fn auto_speak(bot: &Bot, env: &Envelope) {
        let (chance, previous) = {
            let state = bot.state.lock().unwrap();
            (state.chance, state.previous)
        };

        let bypass = if let Some(prev) = previous {
            time::Instant::now().duration_since(prev) > time::Duration::from_secs(60)
        } else {
            false
        };

        if bypass {
            trace!("bypassing the roll");
        }

        if bypass || thread_rng().gen_bool(chance) {
            trace!("automatically trying to speak");
            Bot::speak(bot, env)
        }
    }
}
