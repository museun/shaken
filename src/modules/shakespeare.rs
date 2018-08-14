use rand::prelude::*;
use std::cell::RefCell;
use std::time; // this should be chrono time

use crate::irc::Message;
use crate::*;

pub struct Shakespeare(RefCell<Inner>);

struct Inner {
    previous: Option<time::Instant>,
    limit: time::Duration,
    interval: f64,
    chance: f64,
    bypass: usize,
    name: String,
}

impl Module for Shakespeare {
    fn command(&self, req: &Request) -> Option<Response> {
        if let Some(_req) = req.search("!speak") {
            return self.speak_command();
        }
        None
    }

    fn passive(&self, msg: &Message) -> Option<Response> {
        self.check_mentions(&msg).or_else(|| self.auto_speak())
    }
}

impl Shakespeare {
    pub fn new() -> Self {
        let config = Config::load();

        Self {
            0: RefCell::new(Inner {
                previous: None,
                limit: time::Duration::from_secs(config.shakespeare.interval as u64),
                interval: config.shakespeare.interval as f64,
                chance: config.shakespeare.chance,
                bypass: config.shakespeare.bypass,
                name: config.twitch.name,
            }),
        }
    }

    fn speak_command(&self) -> Option<Response> {
        let resp = self.generate()?;
        say!("{}", resp)
    }

    fn auto_speak(&self) -> Option<Response> {
        let bypass = if let Some(prev) = self.0.borrow().previous {
            time::Instant::now().duration_since(prev)  // don't format this
            > time::Duration::from_secs(self.0.borrow().bypass as u64)
        } else {
            self.0.borrow().bypass == 0
        };

        if bypass {
            trace!("bypassing the roll");
        }

        if bypass || thread_rng().gen_bool(self.0.borrow().chance) {
            trace!("automatically trying to speak");
            return self.speak_command();
        }
        None
    }

    fn check_mentions(&self, msg: &Message) -> Option<Response> {
        let conn = database::get_connection();
        let user = UserStore::get_bot(&conn, &self.0.borrow().name)?;

        fn trim_then_check(s: &str, nick: &str) -> bool {
            let s = s.trim_right_matches(|c: char| !c.is_ascii_alphanumeric());
            !s.is_empty() && s[1..].eq_ignore_ascii_case(nick)
        }

        for part in msg.data.split_whitespace() {
            warn!("part: '{}'", part);
            if part.starts_with('@') && trim_then_check(&part, &user.display) {
                trace!("got a mention, trying to speak");
                return self.speak_command();
            }
        }

        None
    }

    fn generate(&self) -> Option<String> {
        let now = time::Instant::now();
        if let Some(prev) = self.0.borrow().previous {
            if now.duration_since(prev) < self.0.borrow().limit {
                let dur = now.duration_since(prev);
                let rem = self.0.borrow().interval
                    - (dur.as_secs() as f64  // don't format this
                    + f64::from(dur.subsec_nanos()) * 1e-9);
                debug!("already spoke: {:.3}s remaining", rem);
                return None;
            }
        }

        trace!("before conditional");
        #[cfg(not(test))]
        {
            if let Some(data) = crate::util::http_get("http://localhost:7878/markov/next") {
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

                trace!("generated a message");
                self.0.borrow_mut().previous = Some(now);
                Some(prune(&data).to_string() + ".")
            } else {
                warn!("cannot get a response from the brain");
                None
            }
        }
        #[cfg(test)]
        {
            self.0.borrow_mut().previous = Some(now);
            Some("Friends, Romans, countrymen, lend me your ears.".into())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::*;

    #[test]
    fn speak_command() {
        let shakespeare: Box<dyn Module> = Box::new(Shakespeare::new());
        let mut env = Environment::new();
        env.add(&shakespeare);

        env.push("!speak");
        env.step();

        assert_eq!(
            env.pop(),
            Some("Friends, Romans, countrymen, lend me your ears.".into())
        );

        env.push("!speak");
        env.step();

        assert_eq!(env.pop(), None);
    }

    // this always bypasses the roll
    #[test]
    fn auto_speak() {
        let ss = Shakespeare::new();
        {
            ss.0.borrow_mut().bypass = 0;
        }
        let shakespeare: Box<dyn Module> = Box::new(ss);

        let mut env = Environment::new();
        env.add(&shakespeare);

        env.push("testing this out");
        env.step();

        assert_eq!(
            env.pop(),
            Some("Friends, Romans, countrymen, lend me your ears.".into())
        );
    }

    #[test]
    fn check_mentions() {
        let shakespeare: Box<dyn Module> = Box::new(Shakespeare::new());
        let mut env = Environment::new();
        env.add(&shakespeare);

        env.push("hey @shaken_bot");
        env.step();

        assert_eq!(
            env.pop(),
            Some("Friends, Romans, countrymen, lend me your ears.".into())
        );

        env.push("@shaken");
        env.step();

        assert_eq!(env.pop(), None);
    }
}
