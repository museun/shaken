use rand::prelude::*;

use std::sync::{Arc, RwLock};
use std::time;

use {bot, config, message, util::http_get};

pub struct Shakespeare {
    previous: Option<time::Instant>,
    limit: time::Duration,
    interval: f64,
    chance: f64,
    bypass: usize,
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
            bot.say(&env, &resp)
        }
    }

    fn check_mentions(&mut self, bot: &bot::Bot, env: &message::Envelope) {
        let nick = bot.nick();
        trace!("my nick is {}", nick);

        fn trim_then_check(s: &str, nick: &str) -> bool {
            let s = s.to_string();
            let s = s.trim_right_matches(|c: char| !c.is_ascii_alphanumeric());
            !s.is_empty() && &s[1..] == nick
        }

        for part in env.data.split_whitespace() {
            if part.starts_with('@') && trim_then_check(&part, &nick) {
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

        if let Some(data) = http_get("http://localhost:7878/markov/next") {
            trace!("generated a message");
            self.previous = Some(now);
            Some(prune(&data).to_string() + ".")
        } else {
            warn!("cannot get a response from the brain");
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
