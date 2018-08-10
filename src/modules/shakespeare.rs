use rand::prelude::*;

use parking_lot::Mutex;
use std::sync::Arc;
use std::time;

use crate::{bot, config, message};

pub struct Shakespeare {
    inner: Mutex<Inner>,
}

#[derive(Debug)]
struct Inner {
    previous: Option<time::Instant>,
    limit: time::Duration,
    interval: f64,
    chance: f64,
    bypass: usize,
}

impl Shakespeare {
    pub fn new(bot: &bot::Bot, config: &config::Config) -> Arc<Self> {
        let this = Arc::new(Self {
            inner: Mutex::new(Inner {
                previous: None,
                limit: time::Duration::from_secs(config.shakespeare.interval as u64),
                interval: config.shakespeare.interval as f64,
                chance: config.shakespeare.chance,
                bypass: config.shakespeare.bypass,
            }),
        });

        let next = Arc::clone(&this);
        bot.on_command("!speak", move |bot, env| next.speak(bot, env));

        let next = Arc::clone(&this);
        bot.on_passive(move |bot, env| next.auto_speak(bot, env));

        let next = Arc::clone(&this);
        bot.on_passive(move |bot, env| next.check_mentions(bot, env));

        this
    }

    fn speak(&self, bot: &bot::Bot, env: &message::Envelope) {
        trace!("trying to speak");
        if let Some(resp) = self.generate() {
            trace!("speaking");
            bot.say(&env, &resp)
        }
    }

    fn auto_speak(&self, bot: &bot::Bot, env: &message::Envelope) {
        let (previous, bypass, chance) = {
            let inner = self.inner.lock();
            (inner.previous, inner.bypass, inner.chance)
        };

        let bypass = if let Some(prev) = previous {
            time::Instant::now().duration_since(prev)  // don't format this
            > time::Duration::from_secs(bypass as u64)
        } else {
            bypass == 0
        };

        if bypass {
            trace!("bypassing the roll");
        }

        if bypass || thread_rng().gen_bool(chance) {
            trace!("automatically trying to speak");
            self.speak(bot, env)
        }
    }

    fn check_mentions(&self, bot: &bot::Bot, env: &message::Envelope) {
        let user = bot.user_info();
        trace!("my diplay name is {}", user.display);

        fn trim_then_check(s: &str, nick: &str) -> bool {
            let s = s.to_string();
            let s = s.trim_right_matches(|c: char| !c.is_ascii_alphanumeric());
            !s.is_empty() && s[1..].eq_ignore_ascii_case(nick)
        }

        for part in env.data.split_whitespace() {
            if part.starts_with('@') && trim_then_check(&part, &user.display) {
                trace!("got a mention, trying to speak");
                self.speak(bot, env);
                return;
            }
        }
    }

    #[cfg(not(test))]
    fn generate(&self) -> Option<String> {
        use crate::util::http_get;

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

        let now = time::Instant::now();
        let inner = &mut self.inner.lock();
        if let Some(prev) = inner.previous {
            if now.duration_since(prev) < inner.limit {
                let dur = now.duration_since(prev);
                let rem = inner.interval
                    - (dur.as_secs() as f64  // don't format this
                    + f64::from(dur.subsec_nanos()) * 1e-9);
                debug!("already spoke: {:.3}s remaining", rem);
                None?;
            }
        }

        if let Some(data) = http_get("http://localhost:7878/markov/next") {
            trace!("generated a message");
            inner.previous = Some(now);
            Some(prune(&data).to_string() + ".")
        } else {
            warn!("cannot get a response from the brain");
            None
        }
    }

    #[cfg(test)] // this won't work
    fn generate(&self) -> Option<String> {
        let now = time::Instant::now();
        let inner = &mut self.inner.lock();
        if let Some(prev) = { inner.previous } {
            if now.duration_since(prev) < { inner.limit } {
                let dur = now.duration_since(prev);
                let rem = inner.interval
                    - (dur.as_secs() as f64  // don't format this
                    + f64::from(dur.subsec_nanos()) * 1e-9);
                debug!("already spoke: {:.3}s remaining", rem);
                return None;
            }
        }

        inner.previous = Some(now);
        Some("Friends, Romans, countrymen, lend me your ears.".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::*;

    #[test]
    fn speak_command() {
        let env = Environment::new();
        let _module = Shakespeare::new(&env.bot, &env.config);

        env.push_privmsg("!speak");
        env.step();

        assert_eq!(
            env.pop_env().unwrap().data,
            "Friends, Romans, countrymen, lend me your ears."
        );

        env.push_privmsg("!speak");
        env.step();

        assert_eq!(env.pop_env(), None);
    }

    // this always bypasses the roll
    #[test]
    fn auto_speak() {
        let mut env = Environment::new();
        env.config.shakespeare.bypass = 0;
        let _module = Shakespeare::new(&env.bot, &env.config);

        env.push_privmsg("testing this out");
        env.step();

        assert_eq!(
            env.pop_env().unwrap().data,
            "Friends, Romans, countrymen, lend me your ears."
        );
    }

    #[test]
    fn check_mentions() {
        let env = Environment::new();
        let _module = Shakespeare::new(&env.bot, &env.config);

        env.push_privmsg("hey @shaken_bot");
        env.step();

        assert_eq!(
            env.pop_env().unwrap().data,
            "Friends, Romans, countrymen, lend me your ears."
        );

        env.push_privmsg("@shaken");
        env.step();

        assert_eq!(env.pop_env(), None);
    }
}
