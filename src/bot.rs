use config::Config;
use conn::Proto;
use message::{Envelope, Handler, Message};

use std::sync::Mutex;
use std::time;

use curl::easy::Easy;
use rand::prelude::*;

pub struct Bot<'a> {
    proto: Box<Proto + 'a>,
    state: Mutex<State>,
    channels: Vec<String>,
}

impl<'a> Bot<'a> {
    pub fn new(proto: impl Proto + 'a, config: &Config) -> Self {
        Self {
            proto: Box::new(proto),
            state: Mutex::new(State::new(config.interval, config.chance)),
            channels: config.channels.to_vec(),
        }
    }

    pub fn run(&self, config: &Config) {
        self.proto.send(&format!("PASS {}", &config.pass));
        self.proto.send(&format!("NICK {}", &config.nick));
        // this is needed for real irc servers
        self.proto
            .send(&format!("USER {} * 8 :{}", &config.nick, &config.nick));

        let handlers = vec![
            Handler::Active("!speak", Bot::speak),
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
            debug!("{}", msg);

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
                            debug!("running server: {}", cmd);
                            f(&self, &msg)
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    fn speak(bot: &Bot, env: &Envelope) {
        trace!("trying to speak");
        if let Some(resp) = bot.state.lock().unwrap().generate() {
            trace!("speaking");
            bot.proto.privmsg(&env.channel, &resp)
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

struct State {
    previous: Option<time::Instant>,
    limit: time::Duration,
    interval: f64,
    chance: f64,
}

impl State {
    pub fn new(interval: usize, chance: f64) -> Self {
        State {
            previous: None,
            limit: time::Duration::from_secs(interval as u64),
            interval: interval as f64,
            chance,
        }
    }

    pub fn generate(&mut self) -> Option<String> {
        let now = time::Instant::now();
        if let Some(prev) = self.previous {
            if now.duration_since(prev) < self.limit {
                let dur = now.duration_since(prev);
                let rem =
                    self.interval - (dur.as_secs() as f64 + f64::from(dur.subsec_nanos()) * 1e-9);
                debug!("already spoke: {:.3}s remaining", rem);
                None?;
            }
        }

        if let Some(data) = get("http://localhost:7878/markov/next") {
            trace!("generated a message");
            self.previous = Some(now);
            // has to be put in a string anyway
            Some(prune(&data).to_string() + ".")
        } else {
            None
        }
    }
}

fn prune(s: &str) -> &str {
    let mut pos = 0;
    for c in s.chars().rev() {
        if c.is_alphabetic() {
            break;
        }
        pos += 1
    }
    &s[..s.len() - pos]
}

fn get(url: &str) -> Option<String> {
    let mut vec = Vec::new();
    let mut easy = Easy::new();
    easy.url(url).ok()?;
    {
        let mut transfer = easy.transfer();
        let _ = transfer.write_function(|data| {
            vec.extend_from_slice(data);
            Ok(data.len())
        });
        transfer.perform().ok()?;
    }
    String::from_utf8(vec).ok()
}
