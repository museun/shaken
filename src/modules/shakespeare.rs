use crate::prelude::*;
use rand::prelude::*;
use std::time::{Duration, Instant};

pub trait Markov: Send {
    fn get_next(&self) -> Option<String>;
}

pub struct NullMarkov;
impl Markov for NullMarkov {
    fn get_next(&self) -> Option<String> {
        None
    }
}

use std::borrow::Cow;
pub struct BrainMarkov<'a>(pub Cow<'a, str>);
impl<'a> Markov for BrainMarkov<'a> {
    fn get_next(&self) -> Option<String> {
        util::http_get(&self.0)
    }
}

pub struct Shakespeare {
    map: CommandMap<Shakespeare>,
    markov: Box<dyn Markov>,

    previous: Option<Instant>,
    limit: Duration,
    interval: f64,
    chance: f64,
    bypass: usize,
    name: String,
}

impl Module for Shakespeare {
    fn command(&mut self, req: &Request) -> Option<Response> {
        let map = self.map.shallow_clone();
        map.dispatch(self, req)
    }

    fn passive(&mut self, msg: &irc::Message) -> Option<Response> {
        self.check_mentions(&msg).or_else(|| self.auto_speak())
    }
}

impl Shakespeare {
    pub fn create<M: Markov + 'static>(markov: M) -> Result<Self, ModuleError> {
        let map = CommandMap::create("Shakespeare", &[("!speak", Self::speak_command)])?;
        let config = Config::load();

        Ok(Self {
            map,
            markov: Box::new(markov),

            previous: None,
            limit: Duration::from_secs(config.shakespeare.interval as u64),
            interval: config.shakespeare.interval as f64,
            chance: config.shakespeare.chance,
            bypass: config.shakespeare.bypass,
            name: config.twitch.name,
        })
    }

    fn speak_command(&mut self, _: &Request) -> Option<Response> {
        let resp = self.generate()?;
        say!("{}", resp)
    }

    fn auto_speak(&mut self) -> Option<Response> {
        let bypass = if let Some(prev) = self.previous {
            let left = Instant::now().duration_since(prev);
            let right = Duration::from_secs(self.bypass as u64);
            left > right
        } else {
            self.bypass == 0
        };

        if bypass {
            trace!("bypassing the roll");
        }

        if bypass || thread_rng().gen_bool(self.chance) {
            trace!("automatically trying to speak");
            let resp = self.generate()?;
            return say!("{}", resp);
        }
        None
    }

    fn check_mentions(&mut self, msg: &irc::Message) -> Option<Response> {
        let conn = get_connection();
        let user = UserStore::get_bot(&conn, &self.name)?;

        // what is this
        fn trim_then_check(s: &str, nick: &str) -> bool {
            let s = s.trim_right_matches(|c: char| !c.is_ascii_alphanumeric());
            !s.is_empty() && s[1..].eq_ignore_ascii_case(nick)
        }

        for part in msg.data.split_whitespace() {
            if part.starts_with('@') && trim_then_check(&part, &user.display) {
                trace!("got a mention, trying to speak");
                let resp = self.generate()?;
                return say!("{}", resp);
            }
        }
        None
    }

    fn generate(&mut self) -> Option<String> {
        let now = Instant::now();
        if let Some(prev) = self.previous {
            if now.duration_since(prev) < self.limit {
                let (secs, nanos) = {
                    let dur = now.duration_since(prev);
                    (dur.as_secs() as f64, f64::from(dur.subsec_nanos()) * 1e-9)
                };

                let rem = self.interval - secs + nanos;
                debug!("already spoke: {:.3}s remaining", rem);
                return None;
            }
        }

        loop {
            let data = match self.markov.get_next() {
                Some(data) => data,
                None => {
                    warn!("cannot get a response from the brain");
                    return None;
                }
            };

            trace!("generated a message");
            self.previous = Some(now);

            let data = prune(&data).to_string();
            if data.chars().filter(char::is_ascii_whitespace).count() < 3 {
                trace!("trying for a better sentence");
                continue;
            }

            return Some(data + ".");
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::*;

    struct TestMarkov;
    impl Markov for TestMarkov {
        fn get_next(&self) -> Option<String> {
            Some("Friends, Romans, countrymen, lend me your ears.".into())
        }
    }

    #[test]
    fn speak_command() {
        let db = database::get_connection();
        let mut shakespeare = Shakespeare::create(TestMarkov {}).unwrap();
        let mut env = Environment::new(&db, &mut shakespeare);

        env.push("!speak");
        env.step();
        assert_eq!(
            env.pop().unwrap(),
            "Friends, Romans, countrymen, lend me your ears."
        );

        trace!("trying again");
        env.push("!speak");
        env.step_wait(false);
        assert_eq!(env.pop(), None);
    }

    #[test]
    fn auto_speak() {
        let db = database::get_connection();
        let mut shakespeare = Shakespeare::create(TestMarkov {}).unwrap();
        shakespeare.bypass = 0;
        let mut env = Environment::new(&db, &mut shakespeare);

        env.push("testing this out");
        env.step();
        assert_eq!(
            env.pop().unwrap(),
            "Friends, Romans, countrymen, lend me your ears."
        );
    }

    #[test]
    fn check_mentions() {
        let db = database::get_connection();
        let mut shakespeare = Shakespeare::create(TestMarkov {}).unwrap();
        let mut env = Environment::new(&db, &mut shakespeare);

        env.push("hey @shaken_bot");
        env.step();
        assert_eq!(
            env.pop().unwrap(),
            "Friends, Romans, countrymen, lend me your ears."
        );

        env.push("@shaken");
        env.step_wait(false);
        assert_eq!(env.pop(), None);
    }
}
