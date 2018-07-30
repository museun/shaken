use parking_lot::{Mutex, RwLock};
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use std::{fs, str};

use crate::{bot, config, humanize::*, message, twitch::*};

pub struct IdleThing {
    inner: RwLock<Inner>,
    twitch: TwitchClient,
    tick: Mutex<Instant>,
    internal: AtomicUsize,
}

struct Inner {
    state: IdleThingState,
    limit: HashMap<String, Instant>,
}

impl IdleThing {
    pub fn new(bot: &bot::Bot, config: &config::Config) -> Arc<Self> {
        let this = Arc::new(Self {
            inner: RwLock::new(Inner {
                state: IdleThingState::load(&config),
                limit: HashMap::new(),
            }),
            twitch: TwitchClient::new(),
            tick: Mutex::new(Instant::now()),
            internal: AtomicUsize::new(config.idlething.interval),
        });

        let next = Arc::clone(&this);
        bot.on_command("!invest", move |bot, env| {
            next.invest_command(bot, env);
            None
        });

        let next = Arc::clone(&this);
        bot.on_command("!give", move |bot, env| {
            next.give_command(bot, env);
            None
        });

        let next = Arc::clone(&this);
        bot.on_command("!check", move |bot, env| {
            next.check_command(bot, env);
            None
        });

        let next = Arc::clone(&this);
        bot.on_command("!top5", move |bot, env| {
            next.top_command(bot, env);
            None
        });

        let next = Arc::clone(&this);
        bot.on_passive(move |bot, env| {
            next.on_message(bot, env);
            None
        });

        let next = Arc::clone(&this);
        bot.on_tick(move |bot| {
            next.on_tick(bot);
            None
        });

        this
    }

    fn on_tick(&self, bot: &bot::Bot) {
        let now = Instant::now();
        let then = { *self.tick.lock() };
        let interval = self.internal.load(Ordering::Relaxed) as u64;
        if now - then < Duration::from_secs(interval) {
            return;
        }

        {
            let mut then = self.tick.lock();
            *then = now;
        }

        let user = bot.user_info();
        let ch = &bot.channel;

        trace!("getting names for #{}", &ch);
        if let Some(names) = get_names_for(&ch) {
            let mut v = Vec::with_capacity(names.chatter_count);
            v.extend(names.chatters.moderators);
            v.extend(names.chatters.staff);
            v.extend(names.chatters.admins);
            v.extend(names.chatters.global_mods);
            v.extend(names.chatters.viewers);

            // remove the bot from the list
            if let Some(n) = v.iter().position(|s| s.eq_ignore_ascii_case(&user.display)) {
                v.remove(n);
            }

            trace!("names for {}: {:?}", &ch, &v);
            if let Some(users) = self.twitch.get_users(&v) {
                let mut vec = vec![];
                for user in &users {
                    vec.push(user.id.to_string())
                }

                trace!("ids: {:?}", &vec);
                self.inner.write().state.tick(&vec);
            }
        }

        self.inner.write().state.save();
    }

    fn check_limit(&self, who: &str) -> bool {
        if let Some(t) = self.inner.read().limit.get(&who.to_string()) {
            if Instant::now() - *t < Duration::from_secs(60) {
                return true;
            }
        }
        false
    }

    fn rate_limit(&self, who: &str) {
        let who = who.to_string();
        self.inner.write().limit.insert(who, Instant::now());
    }

    fn invest_command(&self, bot: &bot::Bot, env: &message::Envelope) {
        let who = match env.get_id() {
            Some(who) => who,
            None => return,
        };

        if self.check_limit(&who) {
            debug!("{} has been rate limited", who);
            return;
        }

        if let Some(num) = Self::parse_number(&env.data) {
            if num == 0 {
                bot.reply(&env, "zero what?");
                return;
            }

            let state = {
                let state = &mut self.inner.write().state;
                state.invest(who, num)
            };

            match state {
                Ok(s) => match s {
                    Donation::Success { old, new } => {
                        bot.reply(
                            &env,
                            &format!(
                                "success! {} -> {}",
                                old.comma_separate(),
                                new.comma_separate()
                            ),
                        );
                    }
                    Donation::Failure { old, new } => {
                        bot.reply(
                            &env,
                            &format!(
                                "failure! {} -> {}. try again in a minute",
                                old.comma_separate(),
                                new.comma_separate()
                            ),
                        );
                        // rate limit them after they've invested
                        self.rate_limit(who);
                    }
                },
                Err(err) => match err {
                    IdleThingError::NotEnoughCredits { have, want } => {
                        bot.reply(&env, &format!("you don't have enough. you have {} but you want to invest {} credits", have.comma_separate(), want.comma_separate()));
                    }
                },
            }
        } else {
            bot.reply(&env, "thats not a number I understand");
        }
    }

    fn lookup_id_for(&self, name: &str) -> Option<String> {
        if let Some(list) = self.twitch.get_users(&[name]) {
            if let Some(item) = list.get(0) {
                return Some(item.id.to_string());
            }
        }
        None
    }

    fn lookup_display_for<S, V>(&self, ids: V) -> Option<Vec<(String, String)>>
    where
        S: AsRef<str>,
        V: AsRef<[S]>,
        S: ::std::fmt::Debug,
    {
        if let Some(list) = self.twitch.get_users_from_ids(ids.as_ref()) {
            return Some(
                list.iter()
                    .map(|user| (user.display_name.clone(), user.id.clone()))
                    .collect(),
            );
        }
        None
    }

    fn give_command(&self, bot: &bot::Bot, env: &message::Envelope) {
        // TODO determine if these names should be case folded for simpler comparisons
        let who = match env.get_id() {
            Some(who) => who,
            None => return,
        };

        let sender = match env.get_nick() {
            Some(sender) => sender,
            None => return,
        };

        let (mut target, data) = match env.data.split_whitespace().take(1).next() {
            Some(target) => (target, &env.data[target.len()..]),
            None => {
                bot.reply(&env, "who do you want to give points to?");
                return;
            }
        };

        // trim the potential '@'
        if target.starts_with('@') {
            target = &target[1..]
        }

        if target.eq_ignore_ascii_case(&bot.user_info().display) {
            bot.reply(&env, "I don't want any credits.");
            return;
        }

        let tid = match self.lookup_id_for(&target) {
            Some(id) => id,
            None => {
                bot.reply(&env, "I don't know who that is");
                return;
            }
        };

        if sender.eq_ignore_ascii_case(target) {
            bot.reply(&env, "what are you doing?");
            return;
        }

        if let Some(num) = Self::parse_number(data.trim()) {
            if num == 0 {
                bot.reply(&env, "zero what?");
                return;
            }

            debug!("{} wants to give {} {} credits", who, tid, num);

            if let Some(credits) = {
                let inner = self.inner.read();
                let state = &inner.state;
                state.get_credits_for(&who)
            } {
                if num <= credits {
                    let (c, d) = {
                        let mut state = &mut self.inner.write().state;
                        let c = state.give(&tid, num);
                        let d = state.take(&who, num);
                        (c, d)
                    };

                    bot.reply(
                        &env,
                        &format!(
                            "they now have {} credits and you have {}",
                            c.comma_separate(),
                            d.comma_separate()
                        ),
                    );
                } else {
                    bot.reply(
                        &env,
                        &format!("you only have {} credits", credits.comma_separate()),
                    );
                }
            } else {
                bot.reply(&env, "you have no credits")
            }
        } else {
            bot.reply(&env, "how much is that?");
        }
    }

    fn check_command(&self, bot: &bot::Bot, env: &message::Envelope) {
        let who = match env.get_id() {
            Some(who) => who,
            None => return,
        };

        if let Some(credits) = self.inner.read().state.get_credits_for(&who) {
            bot.reply(
                &env,
                &format!("you have {} credits", credits.comma_separate()),
            )
        } else {
            bot.reply(&env, "you have no credits")
        }
    }

    fn top_command(&self, bot: &bot::Bot, env: &message::Envelope) {
        let sorted = { self.inner.write().state.to_sorted() };

        if let Some(ids) =
            self.lookup_display_for(sorted.iter().take(10).map(|(s, _)| s).collect::<Vec<_>>())
        {
            let mut list = vec![];
            for (i, (name, id)) in ids.iter().enumerate().take(5) {
                let pos = sorted.iter().position(|(i, _)| i == id).unwrap(); // check?
                list.push(format!("(#{}) {}: {}", i + 1, name.clone(), sorted[pos].1));
            }
            let res = crate::util::join_with(list.iter(), ", ");
            bot.reply(&env, &res);
        }
    }

    fn on_message(&self, _bot: &bot::Bot, env: &message::Envelope) {
        if env.data.starts_with('!') || env.data.starts_with('@') {
            return;
        }

        if let Some(who) = env.get_id() {
            self.inner.write().state.increment(&who);
        }
    }

    fn parse_number(data: &str) -> Option<usize> {
        let num: String = data.chars().take_while(char::is_ascii_digit).collect();
        num.parse::<usize>().ok()
    }
}

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

#[cfg(not(test))]
impl Drop for IdleThingState {
    fn drop(&mut self) {
        debug!("saving IdleThing to {}", IDLE_STORE);
        self.save();
    }
}

impl IdleThingState {
    #[cfg(not(test))]
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

    #[cfg(test)]
    pub fn load(_config: &config::Config) -> Self {
        IdleThingState::default()
    }

    pub fn save(&self) {
        let f = fs::File::create(IDLE_STORE).expect("to create file");
        serde_json::to_writer(&f, &self).expect("to serialize struct");
        trace!("saving IdleThing to {}", IDLE_STORE)
    }

    pub fn tick<S: AsRef<str>>(&mut self, names: &[S]) {
        let (idle_value, starting) = (self.idle_value, self.starting);
        for name in names.iter().map(|s| s.as_ref().to_string()) {
            self
                .state
                .entry(name) // I guess I could borrow the heap allocated strings. or use a Cow?
                .and_modify(|p| *p += idle_value)
                .or_insert(starting);
        }
    }

    pub fn give(&mut self, name: &str, credits: Credit) -> Credit {
        self.state
            .entry(name.into())
            .and_modify(|c| *c += credits)
            .or_insert(credits);

        let ch = self.state[name];
        trace!("setting {}'s credits to {}", name, ch);
        ch
    }

    pub fn take(&mut self, name: &str, credits: Credit) -> Credit {
        self.state
            .entry(name.into())
            .and_modify(|c| *c -= credits)
            .or_insert(credits);

        let ch = self.state[name];
        trace!("setting {}'s credits to {}", name, ch);
        ch
    }

    pub fn increment(&mut self, name: &str) -> Credit {
        let line_value = self.line_value;
        self.give(name, line_value)
    }

    pub fn insert(&mut self, name: &str) {
        let starting = self.starting;

        match self.state.insert(name.to_owned(), starting) {
            Some(old) => warn!("{} already existed ({})", &name, old),
            None => debug!("new nick added: {}", &name),
        }
    }

    fn invest_success(
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

    fn invest_failure(
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

    #[cfg(not(test))]
    fn try_donation(
        &mut self,
        name: &str,
        have: usize,
        want: usize,
    ) -> Result<Donation, IdleThingError> {
        use rand::prelude::*;

        if have == 0 || want > have {
            Err(IdleThingError::NotEnoughCredits { have, want })?
        }

        if thread_rng().gen_bool(1.0 / 2.0) {
            self.invest_failure(name, have, want)
        } else {
            self.invest_success(name, have, want)
        }
    }

    #[cfg(test)]
    fn try_donation(
        &mut self,
        name: &str,
        have: usize,
        want: usize,
    ) -> Result<Donation, IdleThingError> {
        if have == 0 || want > have {
            Err(IdleThingError::NotEnoughCredits { have, want })?
        }

        self.invest_failure(name, have, want)
    }

    pub fn invest(&mut self, name: &str, want: Credit) -> Result<Donation, IdleThingError> {
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

    pub fn to_sorted(&self) -> Vec<(String, Credit)> {
        let mut sorted = self
            .state
            .iter()
            .map(|(k, v)| (k.to_string(), *v))
            .collect::<Vec<_>>();
        sorted.sort_by(|l, r| r.1.cmp(&l.1));
        // should also sort it by name if equal points
        sorted
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::*;
    use std::mem;

    #[test]
    fn test_invest_command() {
        let env = Environment::new();
        let _module = IdleThing::new(&env.bot, &env.config);

        env.push_privmsg("!invest");
        env.step();

        assert_eq!(
            env.pop_env().unwrap().data,
            "@test: thats not a number I understand"
        );

        env.push_privmsg("!invest 10");
        env.step();

        assert_eq!(
            env.pop_env().unwrap().data,
            "@test: you don't have enough. you have 0 but you want to invest 10 credits"
        );

        {
            _module.inner.write().state.give("1004", 1000);
        }

        env.push_privmsg("!invest 500");
        env.step();

        assert_eq!(
            env.pop_env().unwrap().data,
            "@test: failure! 1,000 -> 500. try again in a minute"
        );
    }

    #[test]
    fn test_give_command() {
        let env = Environment::new();
        let _module = IdleThing::new(&env.bot, &env.config);

        env.push_privmsg("!give");
        env.step();

        assert_eq!(
            env.pop_env().unwrap().data,
            "@test: who do you want to give points to?"
        );

        env.push_privmsg("!give test");
        env.step();

        assert_eq!(env.pop_env().unwrap().data, "@test: what are you doing?");

        env.push_privmsg("!give test 10");
        env.step();

        assert_eq!(env.pop_env().unwrap().data, "@test: what are you doing?");

        {
            _module.inner.write().state.give("1004", 1000);
        }

        env.push_privmsg("!give shaken_bot 10");
        env.step();

        assert_eq!(
            env.pop_env().unwrap().data,
            "@test: I don't want any credits."
        );

        env.push_privmsg("!give museun 10");
        env.step();

        assert_eq!(
            env.pop_env().unwrap().data,
            "@test: they now have 10 credits and you have 990"
        );
    }

    #[test]
    fn test_check_command() {
        let env = Environment::new();
        let _module = IdleThing::new(&env.bot, &env.config);

        env.push_privmsg("!check");
        env.step();

        assert_eq!(env.pop_env().unwrap().data, "@test: you have no credits");

        {
            _module.inner.write().state.give("1004", 1000);
        }

        env.push_privmsg("!check");
        env.step();

        assert_eq!(env.pop_env().unwrap().data, "@test: you have 1,000 credits");
    }

    #[test]
    #[ignore] // this requires too much twitch stuff
    fn test_top5_command() {}

    #[test]
    #[ignore] // this requires too much twitch stuff
    fn test_on_message() {}

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
    fn test_invest() {
        let mut ch = IdleThingState::default();

        assert_eq!(
            ch.invest("test", 10),
            Err(IdleThingError::NotEnoughCredits { have: 0, want: 10 })
        ); // not seen before. so zero credits

        assert_eq!(ch.increment("foo"), 5); // starts at 0, so +5
        assert_eq!(ch.increment("foo"), 10); // then +5

        assert_eq!(
            ch.invest("test", 10),
            Err(IdleThingError::NotEnoughCredits { have: 0, want: 10 })
        ); // not seen before. so zero credits

        assert_eq!(
            ch.invest_success("foo", 10, 5),
            Ok(Donation::Success { old: 10, new: 15 })
        );

        assert_eq!(
            ch.invest_success("foo", 15, 15),
            Ok(Donation::Success { old: 15, new: 30 })
        );

        assert_eq!(
            ch.invest_failure("foo", 30, 15),
            Ok(Donation::Failure { old: 30, new: 15 })
        );

        assert_eq!(
            ch.invest_failure("foo", 15, 15),
            Ok(Donation::Failure { old: 15, new: 0 })
        );

        mem::forget(ch); // don't serialize to disk
    }
}
