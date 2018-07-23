#![allow(dead_code, unused_variables)]

use std::collections::HashMap;
use std::fs;
use std::str;

use rand::prelude::*;

/* plans:
keep track of the user color
keep track of the user display name
does the charity have a 'bank?'
is there a chance to "take" the bank?
*/

const CHARITY_STORE: &str = "charity.json";

type Credit = usize;

const DEFAULT_VALUE: Credit = 0;
const LINE_VALUE: Credit = 5;
const IDLE_VALUE: Credit = 1;

#[derive(Debug, PartialEq)]
pub enum CharityError {
    NotEnoughCredits { have: Credit, want: Credit },
    // a rate limit error?
}

#[derive(Debug, PartialEq)]
pub enum Donation {
    Success { old: Credit, new: Credit },
    Failure { old: Credit, new: Credit },
}

#[derive(Debug, Deserialize, Serialize, Default)]
pub struct Charity {
    state: HashMap<String, Credit>, // this has to own the strings
}

impl Drop for Charity {
    fn drop(&mut self) {
        let f = fs::File::create(CHARITY_STORE).expect("to create file");
        serde_json::to_writer(&f, &self).expect("to serialize struct");
        debug!("saving charity to {}", CHARITY_STORE)
    }
}

impl Charity {
    pub fn load() -> Self {
        debug!("loading charity from: {}", CHARITY_STORE);
        let s = fs::read_to_string(CHARITY_STORE)
            .or_else(|_| {
                debug!("loading default charity");
                serde_json::to_string_pretty(&Charity::default())
            })
            .expect("to get json");
        serde_json::from_str(&s).expect("to deserialize struct")
    }

    pub fn tick(&mut self, names: &[&str]) {
        for name in names.iter().map(|s| s.to_string()) {
            let copy = name.to_string(); // this is expensive
            let new = self
                .state
                .entry(name) // I guess I could borrow the heap allocated strings. or use a Cow?
                .and_modify(|p| *p += IDLE_VALUE)
                .or_insert(DEFAULT_VALUE);
            trace!("tick: incrementing {}'s credits to {}", &copy, new)
        }
    }

    pub fn increment(&mut self, name: &str) -> Credit {
        self.state
            .entry(name.into())
            .and_modify(|c| *c += LINE_VALUE)
            .or_insert(LINE_VALUE);
        let ch = self.state[name];
        debug!("incrementing {}'s credits, now: {}", name, ch);
        ch
    }

    pub fn insert(&mut self, name: &str) {
        match self.state.insert(name.to_owned(), DEFAULT_VALUE) {
            Some(old) => warn!("{} already existed ({})", &name, old),
            None => info!("new nick added: {}", &name),
        }
    }

    fn donate_success(
        &mut self,
        name: &str,
        have: Credit,
        want: Credit,
    ) -> Result<Donation, CharityError> {
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
    ) -> Result<Donation, CharityError> {
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

    pub fn donate(&mut self, name: &str, want: Credit) -> Result<Donation, CharityError> {
        if let Some(have) = self.get_credits_for(name) {
            // 50/50
            if thread_rng().gen_bool(1.0 / 2.0) {
                self.donate_failure(name, have, want)
            } else {
                self.donate_success(name, have, want)
            }
        } else {
            Err(CharityError::NotEnoughCredits { have: 0, want })
        }
    }

    pub fn get_credits_for(&self, name: &str) -> Option<Credit> {
        self.state.get(name).cloned()
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

    fn dump(ch: &Charity) {
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
        let mut ch = Charity::default();
        let names = vec!["foo", "bar", "baz", "quux"];
        for name in &names {
            ch.insert(&name)
        }
        for name in &names {
            assert_eq!(ch.get_credits_for(&name), Some(0));
        }
        assert_eq!(ch.get_credits_for("test"), None);

        mem::forget(ch); // don't serialize to disk
    }

    #[test]
    fn test_tick() {
        let mut ch = Charity::default();
        ch.insert("foo");
        ch.insert("bar");

        ch.tick(&["foo", "baz", "quux"]);
        assert_eq!(ch.get_credits_for("foo"), Some(1)); // was already there when the tick happened
        assert_eq!(ch.get_credits_for("bar"), Some(0)); // not there when the tick happened
        assert_eq!(ch.get_credits_for("baz"), Some(0)); // new when the tick happened
        assert_eq!(ch.get_credits_for("quux"), Some(0)); // new when the tick happened

        mem::forget(ch); // don't serialize to disk
    }

    #[test]
    fn test_donate() {
        let mut ch = Charity::default();

        assert_eq!(
            ch.donate("test", 10),
            Err(CharityError::NotEnoughCredits { have: 0, want: 10 })
        ); // not seen before. so zero credits

        assert_eq!(ch.increment("foo"), 5); // starts at 0, so +5
        assert_eq!(ch.increment("foo"), 10); // then +5

        assert_eq!(
            ch.donate("test", 10),
            Err(CharityError::NotEnoughCredits { have: 0, want: 10 })
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
