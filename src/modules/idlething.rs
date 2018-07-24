use rand::prelude::*;

use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::thread;
use std::time::Duration;
use std::{fmt::Write, fs, str};

use {bot, config, humanize::*, message};

pub struct IdleThing {
    state: IdleThingState,
}

impl IdleThing {
    pub fn new(bot: &bot::Bot, config: &config::Config) -> Arc<RwLock<Self>> {
        let this = Arc::new(RwLock::new(Self {
            state: IdleThingState::load(&config),
        }));

        let next = Arc::clone(&this);
        bot.on_command("!donate", move |bot, env| {
            next.write().unwrap().donate_command(bot, env);
        });

        let next = Arc::clone(&this);
        bot.on_command("!give", move |bot, env| {
            next.write().unwrap().give_command(bot, env);
        });

        let next = Arc::clone(&this);
        bot.on_command("!check", move |bot, env| {
            next.write().unwrap().check_command(bot, env);
        });

        let next = Arc::clone(&this);
        bot.on_command("!top5", move |bot, env| {
            next.write().unwrap().top_command(bot, env);
        });

        let next = Arc::clone(&this);
        bot.on_passive(move |bot, env| {
            next.write().unwrap().on_message(bot, env);
        });

        let dur = Duration::from_secs(config.idlething.interval as u64);
        let next = Arc::clone(&this);

        thread::spawn(move || loop {
            let ch = "museun";
            trace!("getting names for #{}", ch);
            if let Some(names) = bot::get_names_for(ch) {
                let mut v = Vec::with_capacity(names.chatter_count);
                v.extend(names.chatters.moderators);
                v.extend(names.chatters.staff);
                v.extend(names.chatters.admins);
                v.extend(names.chatters.global_mods);
                v.extend(names.chatters.viewers);

                trace!("names for {}: {:?}", &ch, &v);
                next.write().unwrap().state.tick(&v);
            }
            next.write().unwrap().state.save();
            thread::sleep(dur);
        });

        this
    }

    fn donate_command(&mut self, bot: &bot::Bot, env: &message::Envelope) {
        if let Some(who) = env.get_nick() {
            let num: String = env.data.chars().take_while(char::is_ascii_digit).collect();
            if let Ok(num) = num.parse::<usize>() {
                if num == 0 {
                    bot.reply(&env, "zero what?");
                    return;
                }

                match self.state.donate(who, num) {
                    Ok(s) => match s {
                        Donation::Success { old, new } => {
                            bot.reply(&env, &format!("success! you went from {} to {}", old, new));
                        }
                        Donation::Failure { old, new } => {
                            bot.reply(&env, &format!("failure! you went from {} to {}", old, new));
                        }
                    },
                    Err(err) => match err {
                        IdleThingError::NotEnoughCredits { have, want } => {
                            bot.reply(&env, &format!("you don't have enough. you have {} but you want to spend {} credits", have, want));
                        }
                    },
                }
            } else {
                bot.reply(&env, "thats not a number I understand");
            }
        }
    }

    fn give_command(&mut self, bot: &bot::Bot, env: &message::Envelope) {
        if let Some(who) = env.get_nick() {
            if let Some(credits) = self.state.get_credits_for(&who) {
                bot.reply(
                    &env,
                    &format!("you have {} credits", credits.comma_separate()),
                )
            } else {
                bot.reply(&env, "you have no credits")
            }
        }
    }

    fn check_command(&mut self, bot: &bot::Bot, env: &message::Envelope) {
        if let Some(who) = env.get_nick() {
            if let Some(credits) = self.state.get_credits_for(&who) {
                bot.reply(
                    &env,
                    &format!("you have {} credits", credits.comma_separate()),
                )
            } else {
                bot.reply(&env, "you have no credits")
            }
        }
    }

    fn top_command(&mut self, bot: &bot::Bot, env: &message::Envelope) {
        fn comma_join(list: &[(&str, usize)]) -> String {
            let mut buf = String::new();
            for (i, (k, v)) in list.iter().enumerate() {
                write!(&mut buf, "(#{}) {}: {}, ", i + 1, k, v);
            }
            let mut buf = buf.trim();
            if let Some(c) = buf.chars().last() {
                if c == ',' {
                    buf = &buf[..buf.len() - 1]
                }
            }
            buf.to_string()
        }

        let sorted = self.state.to_sorted();
        let res = comma_join(&sorted.iter().take(5).cloned().collect::<Vec<_>>());
        bot.reply(&env, &res);
    }

    fn on_message(&mut self, _bot: &bot::Bot, env: &message::Envelope) {
        if env.data.starts_with('!') || env.data.starts_with('@') {
            return;
        }

        if let Some(who) = env.get_nick() {
            self.state.increment(&who);
        }
    }
}

/* plans:
keep track of the user color
keep track of the user display name
does the IdleThing have a 'bank?'
is there a chance to "take" the bank?
*/

const IDLE_STORE: &str = "idlething.json";

type Credit = usize;

#[derive(Debug, PartialEq)]
pub enum IdleThingError {
    NotEnoughCredits { have: Credit, want: Credit },
    // a rate limit error?
}

#[derive(Debug, PartialEq)]
pub enum Donation {
    Success { old: Credit, new: Credit },
    Failure { old: Credit, new: Credit },
}

#[derive(Debug, Deserialize, Serialize)]
pub struct IdleThingState {
    state: HashMap<String, Credit>, // this has to own the strings

    #[serde(skip)]
    starting: usize,
    #[serde(skip)]
    line_value: usize,
    #[serde(skip)]
    idle_value: usize,
}

impl Default for IdleThingState {
    fn default() -> Self {
        Self {
            starting: 0,
            line_value: 5,
            idle_value: 1,
            state: Default::default(),
        }
    }
}

impl Drop for IdleThingState {
    fn drop(&mut self) {
        debug!("saving IdleThing to {}", IDLE_STORE);
        self.save();
    }
}

impl IdleThingState {
    pub fn load(config: &config::Config) -> Self {
        debug!("loading IdleThing from: {}", IDLE_STORE);
        let s = fs::read_to_string(IDLE_STORE)
            .or_else(|_| {
                debug!("loading default IdleThing");
                serde_json::to_string_pretty(&IdleThingState::default())
            })
            .expect("to get json");
        let mut this: Self = serde_json::from_str(&s).expect("to deserialize struct");
        this.starting = config.idlething.starting;
        this.line_value = config.idlething.line_value;
        this.idle_value = config.idlething.idle_value;
        this
    }

    pub fn save(&self) {
        let f = fs::File::create(IDLE_STORE).expect("to create file");
        serde_json::to_writer(&f, &self).expect("to serialize struct");
        trace!("saving IdleThing to {}", IDLE_STORE)
    }

    pub fn tick<S: AsRef<str>>(&mut self, names: &[S]) {
        let (idle_value, starting) = (self.idle_value, self.starting);

        for name in names.iter().map(|s| s.as_ref().to_string()) {
            let copy = name.to_string(); // this is expensive
            let new = self
                .state
                .entry(name) // I guess I could borrow the heap allocated strings. or use a Cow?
                .and_modify(|p| *p += idle_value)
                .or_insert(starting);
            trace!("tick: incrementing {}'s credits to {}", &copy, new)
        }
    }

    pub fn increment(&mut self, name: &str) -> Credit {
        let line_value = self.line_value;

        self.state
            .entry(name.into())
            .and_modify(|c| *c += line_value)
            .or_insert(line_value);
        let ch = self.state[name];
        debug!("incrementing {}'s credits, now: {}", name, ch);
        ch
    }

    pub fn insert(&mut self, name: &str) {
        let starting = self.starting;

        match self.state.insert(name.to_owned(), starting) {
            Some(old) => warn!("{} already existed ({})", &name, old),
            None => debug!("new nick added: {}", &name),
        }
    }

    fn donate_success(
        &mut self,
        name: &str,
        have: Credit,
        want: Credit,
    ) -> Result<Donation, IdleThingError> {
        self.state.entry(name.into()).and_modify(|c| *c += want);

        let amount = self.state[name];
        debug!("donation was successful: {}, {} -> {}", name, have, amount);
        Ok(Donation::Success {
            old: have,
            new: amount,
        })
    }

    fn donate_failure(
        &mut self,
        name: &str,
        have: Credit,
        want: Credit,
    ) -> Result<Donation, IdleThingError> {
        self.state.entry(name.into()).and_modify(|c| {
            if let Some(v) = c.checked_sub(want) {
                *c = v
            } else {
                *c = 0;
            }
        });

        let amount = self.state[name];
        debug!("donation was a failure: {}, {} -> {}", name, have, amount);
        Ok(Donation::Failure {
            old: have,
            new: amount,
        })
    }

    fn try_donation(
        &mut self,
        name: &str,
        have: usize,
        want: usize,
    ) -> Result<Donation, IdleThingError> {
        if have == 0 || want > have {
            Err(IdleThingError::NotEnoughCredits { have, want })?
        }

        if thread_rng().gen_bool(1.0 / 2.0) {
            self.donate_failure(name, have, want)
        } else {
            self.donate_success(name, have, want)
        }
    }

    pub fn donate(&mut self, name: &str, want: Credit) -> Result<Donation, IdleThingError> {
        if let Some(have) = self.get_credits_for(name) {
            self.try_donation(name, have, want)
        } else {
            Err(IdleThingError::NotEnoughCredits { have: 0, want })
        }
    }

    // returns None if no value, or a 0
    pub fn get_credits_for(&self, name: &str) -> Option<Credit> {
        self.state
            .get(name)
            .and_then(|c| if *c == 0 { None } else { Some(*c) })
    }

    pub fn to_sorted<'a>(&'a self) -> Vec<(&'a str, Credit)> {
        let mut sorted = self
            .state
            .iter()
            .map(|(k, v)| (&**k, *v))
            .collect::<Vec<_>>();
        sorted.sort_by(|l, r| r.1.cmp(&l.1));
        // should also sort it by name if equal points
        sorted
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    extern crate env_logger;
    use humanize::CommaSeparated;
    use std::mem;

    fn dump(ch: &IdleThingState) {
        for (k, v) in ch.to_sorted() {
            debug!("{}: {}", k, v.comma_separate());
        }
    }

    fn init_logger() {
        let _ = env_logger::Builder::from_default_env()
            .default_format_timestamp(false)
            .try_init();
    }

    #[test]
    fn test_default_insert() {
        let mut ch = IdleThingState::default();
        let names = vec!["foo", "bar", "baz", "quux"];
        for name in &names {
            ch.insert(&name)
        }
        for name in &names {
            assert_eq!(ch.get_credits_for(&name), None);
        }
        assert_eq!(ch.get_credits_for("test"), None);

        mem::forget(ch); // don't serialize to disk
    }

    #[test]
    fn test_tick() {
        let mut ch = IdleThingState::default();
        ch.insert("foo");
        ch.insert("bar");

        ch.tick(&["foo", "baz", "quux"]);
        assert_eq!(ch.get_credits_for("foo"), Some(1)); // was already there when the tick happened
        assert_eq!(ch.get_credits_for("bar"), None); // not there when the tick happened
        assert_eq!(ch.get_credits_for("baz"), None); // new when the tick happened
        assert_eq!(ch.get_credits_for("quux"), None); // new when the tick happened

        mem::forget(ch); // don't serialize to disk
    }

    #[test]
    fn test_donate() {
        let mut ch = IdleThingState::default();

        assert_eq!(
            ch.donate("test", 10),
            Err(IdleThingError::NotEnoughCredits { have: 0, want: 10 })
        ); // not seen before. so zero credits

        assert_eq!(ch.increment("foo"), 5); // starts at 0, so +5
        assert_eq!(ch.increment("foo"), 10); // then +5

        assert_eq!(
            ch.donate("test", 10),
            Err(IdleThingError::NotEnoughCredits { have: 0, want: 10 })
        ); // not seen before. so zero credits

        assert_eq!(
            ch.donate_success("foo", 10, 5),
            Ok(Donation::Success { old: 10, new: 15 })
        );

        assert_eq!(
            ch.donate_success("foo", 15, 15),
            Ok(Donation::Success { old: 15, new: 30 })
        );

        assert_eq!(
            ch.donate_failure("foo", 30, 15),
            Ok(Donation::Failure { old: 30, new: 15 })
        );

        assert_eq!(
            ch.donate_failure("foo", 15, 15),
            Ok(Donation::Failure { old: 15, new: 0 })
        );

        mem::forget(ch); // don't serialize to disk
    }
}
