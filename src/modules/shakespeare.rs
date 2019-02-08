use crate::prelude::*;
use log::*;
use rand::prelude::*;
use std::borrow::Cow;
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

pub struct BrainMarkov<'a>(pub Cow<'a, str>);

impl<'a> Markov for BrainMarkov<'a> {
    fn get_next(&self) -> Option<String> {
        util::http_get(&self.0).ok()
    }
}

pub struct Shakespeare {
    map: CommandMap<Shakespeare>,
    markovs: Vec<Box<dyn Markov>>,

    previous: Option<Instant>,
    limit: Duration,
    interval: f64,
    chance: f64,
    bypass: usize, // is this even needed?
    name: String,
}

impl Module for Shakespeare {
    fn command(&mut self, req: &Request) -> Option<Response> {
        let map = self.map.clone();
        map.dispatch(self, req)
    }

    fn passive(&mut self, msg: &irc::Message) -> Option<Response> {
        self.check_mentions(&msg).or_else(|| {
            if !msg.expect_data().starts_with('!') {
                self.auto_speak()
            } else {
                None
            }
        })
    }
}

impl Shakespeare {
    pub fn create(markovs: Vec<Box<dyn Markov>>) -> Result<Self, ModuleError> {
        let map = CommandMap::create(
            "Shakespeare",
            &[
                ("!speak configure", Self::configure_command),
                ("!speak", Self::speak_command),
            ],
        )?;
        let config = Config::load();

        Ok(Self {
            map,
            markovs,

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

    fn configure_command(&mut self, req: &Request) -> Option<Response> {
        require_privileges!(&req);
        let args = req.args_iter().collect::<Vec<_>>();
        if args.is_empty() {
            return reply!("what do you want to configure: interval, chance, bypass");
        }

        let res = match args.as_slice() {
            ["interval", n] => {
                if let Ok(n) = n.parse::<f64>() {
                    self.interval = n;
                    reply!("done")
                } else {
                    reply!("that is not a number")
                }
            }
            ["chance", n] => {
                if let Ok(n) = n.parse::<f64>() {
                    if n > 1.0 || n < 0.0 {
                        return reply!("chance has to be 0.0 <= chance <= 1.0");
                    }
                    self.chance = n;
                    reply!("done")
                } else {
                    reply!("that is not a number")
                }
            }
            ["bypass", n] => {
                if let Ok(n) = n.parse::<usize>() {
                    self.bypass = n;
                    reply!("done")
                } else {
                    reply!("that is not a number")
                }
            }
            ["interval"] | ["chance"] | ["bypass"] => reply!("provide a value, please"),
            _ => reply!("I don't know how to configure that"),
        };

        let mut config = Config::load();
        config.shakespeare.chance = self.chance;
        config.shakespeare.interval = self.interval as usize; // what
        config.shakespeare.bypass = self.bypass;
        config.save();

        res
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
            std::thread::sleep(Duration::from_millis(thread_rng().gen_range(150, 750)));
            return say!("{}", resp);
        }
        None
    }

    fn check_mentions(&mut self, msg: &irc::Message) -> Option<Response> {
        let user = UserStore::get_bot(&get_connection())?;

        fn trim_then_check(s: &str, nick: &str) -> bool {
            let s = s.trim_end_matches(|c: char| !c.is_ascii_alphanumeric());
            !s.is_empty() && s[1..].eq_ignore_ascii_case(nick)
        }

        for part in msg.expect_data().split_whitespace() {
            if part.starts_with('@') && trim_then_check(&part, &user.display) {
                trace!("got a mention, trying to speak");
                let resp = self.generate()?;
                return say!("{}", resp);
            }
        }
        None
    }

    fn generate(&mut self) -> Option<String> {
        self.markovs.shuffle(&mut thread_rng());
        let markov = self.markovs.get(0)?;

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
            let data = match markov.get_next() {
                Some(data) => data,
                None => {
                    warn!("cannot get a response from the brain");
                    return None;
                }
            };

            trace!("generated a message");
            self.previous = Some(now);

            let data = prune(&data);
            if data.chars().filter(char::is_ascii_whitespace).count() < 3 {
                trace!("trying for a better sentence");
                continue;
            }

            return Some([data, "."].concat());
        }
    }
}

fn prune(s: &str) -> &str {
    let pos = s.chars().rev().take_while(|c| !c.is_alphabetic()).count();
    &s[..s.len() - pos] // keep atleast one form of punctuation at the end
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
        let mut shakespeare = Shakespeare::create(vec![Box::new(TestMarkov {})]).unwrap();
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
    fn configure_command() {
        let db = database::get_connection();
        let mut shakespeare = Shakespeare::create(vec![Box::new(TestMarkov {})]).unwrap();
        {
            let mut env = Environment::new(&db, &mut shakespeare);

            env.push("!speak configure");
            env.step_wait(false);
            assert_eq!(env.pop(), None);

            env.push_broadcaster("!speak configure");
            env.step();
            assert_eq!(
                env.pop().unwrap(),
                "@test: what do you want to configure: interval, chance, bypass"
            );

            env.push_broadcaster("!speak configure interval");
            env.step();
            assert_eq!(env.pop().unwrap(), "@test: provide a value, please");

            env.push_broadcaster("!speak configure interval one");
            env.step();
            assert_eq!(env.pop().unwrap(), "@test: that is not a number");

            env.push_broadcaster("!speak configure interval 1");
            env.step();
            assert_eq!(env.pop().unwrap(), "@test: done");

            env.push_broadcaster("!speak configure chance 1.2");
            env.step();
            assert_eq!(
                env.pop().unwrap(),
                "@test: chance has to be 0.0 <= chance <= 1.0"
            );

            env.push_broadcaster("!speak configure chance 0.2");
            env.step();
            assert_eq!(env.pop().unwrap(), "@test: done");

            env.push_broadcaster("!speak configure bypass 1");
            env.step();
            assert_eq!(env.pop().unwrap(), "@test: done");

            env.push_broadcaster("!speak configure foobar 1");
            env.step();
            assert_eq!(
                env.pop().unwrap(),
                "@test: I don't know how to configure that"
            );
        }

        assert_eq!(shakespeare.interval, 1.0);
        assert_eq!(shakespeare.chance, 0.2);
        assert_eq!(shakespeare.bypass, 1);

        // env.drain_and_log();
    }

    #[test]
    fn auto_speak() {
        let db = database::get_connection();
        let mut shakespeare = Shakespeare::create(vec![Box::new(TestMarkov {})]).unwrap();
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
        let mut shakespeare = Shakespeare::create(vec![Box::new(TestMarkov {})]).unwrap();
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
