use curl::easy::Easy;
use rand::prelude::*;

use std::sync::{Arc, RwLock};
use std::time;

use {bot, config, message};

pub struct Shakespeare {
    pub(crate) previous: Option<time::Instant>,
    pub(crate) limit: time::Duration,
    pub(crate) interval: f64,
    pub(crate) chance: f64,
    pub(crate) bypass: usize,
}

impl Shakespeare {
    pub fn new(bot: &bot::Bot, config: &config::Config) -> Arc<RwLock<Self>> {
        let this = Arc::new(RwLock::new(Self {
            previous: None,
            limit: time::Duration::from_secs(config.shakespeare.interval as u64),
            interval: config.shakespeare.interval as f64,
            chance: config.shakespeare.chance,
            bypass: config.shakespeare.bypass,
        }));

        let next = Arc::clone(&this);
        bot.on_command("!speak", move |bot, env| {
            next.write().unwrap().speak(bot, env)
        });

        let next = Arc::clone(&this);
        bot.on_passive(move |bot, env| {
            next.write().unwrap().auto_speak(bot, env);
        });

        let next = Arc::clone(&this);
        bot.on_passive(move |bot, env| {
            next.write().unwrap().check_mentions(bot, env);
        });

        this
    }

    fn speak(&mut self, bot: &bot::Bot, env: &message::Envelope) {
        trace!("trying to speak");
        if let Some(resp) = self.generate() {
            trace!("speaking");
            bot.proto().privmsg(&env.channel, &resp)
        }
    }

    fn check_mentions(&mut self, bot: &bot::Bot, env: &message::Envelope) {
        let nick = bot.nick();
        trace!("my nick is {}", nick);

        for part in env.data.split_whitespace() {
            if part.starts_with('@') && part[1..] == nick {
                trace!("got a mention, trying to speak");
                self.speak(bot, env);
                return;
            }
        }
    }

    fn auto_speak(&mut self, bot: &bot::Bot, env: &message::Envelope) {
        let bypass = if let Some(prev) = self.previous {
            time::Instant::now().duration_since(prev)
                > time::Duration::from_secs(self.bypass as u64)
        } else {
            false
        };

        if bypass {
            trace!("bypassing the roll");
        }

        if bypass || thread_rng().gen_bool(self.chance) {
            trace!("automatically trying to speak");
            self.speak(bot, env)
        }
    }

    fn generate(&mut self) -> Option<String> {
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
