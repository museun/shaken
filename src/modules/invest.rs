use parking_lot::RwLock;
use rand::prelude::*;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use std::{fmt, str};

use crate::{bot, config, humanize::*, message, twitch::*};

pub struct Invest {
    inner: RwLock<Inner>,
    twitch: TwitchClient,
}

struct Inner {
    state: InvestState,
    limit: HashMap<String, Instant>,
}

impl Invest {
    pub fn new(bot: &bot::Bot, config: &config::Config) -> Arc<Self> {
        let this = Arc::new(Self {
            inner: RwLock::new(Inner {
                state: InvestState::load(&config),
                limit: HashMap::new(),
            }),
            twitch: TwitchClient::new(),
        });

        // TODO get rid of this garbage
        let next = Arc::clone(&this);
        bot.on_command("!invest", move |bot, env| next.invest_command(bot, env));

        let next = Arc::clone(&this);
        bot.on_command("!give", move |bot, env| next.give_command(bot, env));

        let next = Arc::clone(&this);
        bot.on_command("!check", move |bot, env| next.check_command(bot, env));

        let next = Arc::clone(&this);
        bot.on_command("!top5", move |bot, env| next.top_command(bot, env));

        let next = Arc::clone(&this);
        bot.on_command("!stats", move |bot, env| next.stats_command(bot, env));

        let next = Arc::clone(&this);
        bot.on_passive(move |bot, env| next.on_message(bot, env));

        this
    }

    fn invest_command(&self, bot: &bot::Bot, env: &message::Envelope) {
        let who = bail!(env.get_id());

        #[cfg(not(test))]
        {
            if self.check_rate_limit(&who) {
                debug!("{} has been rate limited", who);
                return;
            }
        }

        let num = match self.get_credits_from_str(&env.data, &who) {
            Some(num) => num,
            None => {
                bot.reply(&env, "thats not a number I understand");
                return;
            }
        };

        if num == 0 {
            bot.reply(&env, "zero what?");
            return;
        }

        let state = {
            let state = &mut self.inner.write().state;
            state.invest(who, num)
        };

        let response = match state {
            Ok(s) => match s {
                Donation::Success { old, new } => format!(
                    "success! {} -> {}",
                    old.comma_separate(),
                    new.comma_separate()
                ),
                Donation::Failure { old, new } => {
                    // rate limit them after they've failed to invest
                    self.rate_limit(who);
                    format!(
                        "failure! {} -> {}. try again in a minute",
                        old.comma_separate(),
                        new.comma_separate()
                    )
                }
            },
            Err(InvestError::NotEnoughCredits { have, want }) => format!(
                "you don't have enough. you have {}, but you want to invest {} credits",
                have.comma_separate(),
                want.comma_separate()
            ),
        };

        bot.reply(&env, &response);
    }

    fn give_command(&self, bot: &bot::Bot, env: &message::Envelope) {
        // TODO determine if these names should be case folded for simpler comparisons
        let who = bail!(env.get_id());
        let sender = bail!(env.get_nick());

        let (mut target, data) = match env.data.split_whitespace().take(1).next() {
            Some(target) => (target, &env.data[target.len()..]),
            None => {
                bot.reply(&env, "who do you want to give credits to?");
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

        trace!("seeing if '{}' is a valid amounte", &data);

        let num = match self.get_credits_from_str(&data, &who) {
            Some(num) => num,
            None => {
                bot.reply(&env, "thats not a number I understand");
                return;
            }
        };

        if num == 0 {
            bot.reply(&env, "zero what?");
            return;
        }

        debug!("{} wants to give {} {} credits", who, tid, num);

        let credits = match {
            let inner = self.inner.read();
            let state = &inner.state;
            state.get_credits_for(&who)
        } {
            Some(credits) => credits,
            None => {
                bot.reply(&env, "you have no credits");
                return;
            }
        };

        if num > credits {
            bot.reply(
                &env,
                &format!("you only have {} credits", credits.comma_separate()),
            );
            return;
        }

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
    }

    fn check_command(&self, bot: &bot::Bot, env: &message::Envelope) {
        let who = bail!(env.get_id());
        match self.inner.read().state.get_credits_for(&who) {
            Some(credits) if credits > 0 => bot.reply(
                &env,
                &format!("you have {} credits", credits.comma_separate()),
            ),
            _ => bot.reply(&env, "you have no credits"),
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

    fn stats_command(&self, bot: &bot::Bot, env: &message::Envelope) {
        let who = bail!(env.get_id());

        let total = { self.inner.read().state.total };

        let mut inner = self.inner.write();
        let user = inner.state.get(who);
        bot.reply(
            &env,
            &format!(
                "you've {}. and I've 'collected' {} credits",
                user,
                total.comma_separate()
            ),
        );
    }

    fn on_message(&self, _bot: &bot::Bot, env: &message::Envelope) {
        if env.data.starts_with('!') || env.data.starts_with('@') {
            return;
        }

        let who = bail!(env.get_id());

        let mut inner = self.inner.write();
        inner.state.increment(&who, &IncrementType::Line);

        if let Some(kappas) = env.get_emotes() {
            let ty = IncrementType::Emote(kappas.len());
            inner.state.increment(&who, &ty);
        };
    }

    fn lookup_id_for(&self, name: &str) -> Option<String> {
        let list = self.twitch.get_users(&[name])?;
        Some(list.get(0)?.id.to_string())
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

    fn rate_limit(&self, who: &str) {
        let who = who.to_string();
        self.inner.write().limit.insert(who, Instant::now());
    }

    fn check_rate_limit(&self, who: &str) -> bool {
        if let Some(t) = self.inner.read().limit.get(&who.to_string()) {
            if Instant::now() - *t < Duration::from_secs(60) {
                return true;
            }
        }
        false
    }

    fn parse_number_or_context(data: &str) -> Option<NumType> {
        const CONTEXTS: [&str; 3] = ["all", "half", "random"];

        let num: String = data.chars().take_while(char::is_ascii_digit).collect();
        if let Ok(num) = num.parse::<usize>() {
            return Some(NumType::Num(num));
        }

        if let Some(part) = data.split_whitespace().take(1).next() {
            for ctx in &CONTEXTS {
                if &part == ctx {
                    return Some(part.into());
                }
            }
        }

        None
    }

    fn get_credits_from_str(&self, data: &str, id: &str) -> Option<usize> {
        let data = data.trim();

        let num = Self::parse_number_or_context(&data)?;
        let credits = {
            let inner = self.inner.read();
            let state = &inner.state;
            let credits = state.get_credits_for(&id);
            trace!("got {:?} credits for {}", credits, id);
            credits
        }.or(Some(0))?;

        Some(match num {
            NumType::Num(num) => num,
            NumType::All => credits,
            NumType::Half => credits / 2,
            NumType::Random => thread_rng().gen_range(1, credits),
        })
    }
}

enum NumType {
    Num(usize),
    All,
    Half,
    Random,
}

impl From<&str> for NumType {
    fn from(s: &str) -> Self {
        match s {
            "all" => NumType::All,
            "half" => NumType::Half,
            "random" => NumType::Random,
            _ => unreachable!(),
        }
    }
}

const INVEST_STORE: &str = "invest.json";
type Credit = usize;

#[derive(Deserialize, Serialize, Debug)]
struct InvestUser {
    id: String,
    max: usize,
    current: usize,
    total: usize,
    invest: (usize, usize), // success, failure
}

impl InvestUser {
    pub fn new(id: &str) -> InvestUser {
        InvestUser {
            id: id.to_string(),
            max: 0,
            current: 0,
            total: 0,
            invest: (0, 0),
        }
    }
}

impl fmt::Display for InvestUser {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let (success, failure) = self.invest;

        write!(
            f,
            "reached a max of {} credits, out of {} total credits with {} successes and {} failures.",
            self.max.comma_separate(),
            self.total.comma_separate(),
            success.comma_separate(),
            failure.comma_separate(),
        )
    }
}

#[derive(Debug, PartialEq)]
enum InvestError {
    NotEnoughCredits { have: Credit, want: Credit },
    // a rate limit error?
}

#[derive(Debug, PartialEq)]
enum Donation {
    Success { old: Credit, new: Credit },
    Failure { old: Credit, new: Credit },
}

type InvestResult = Result<Donation, InvestError>;

#[derive(Debug, Deserialize, Serialize)]
struct InvestState {
    total: usize, // total lost
    state: HashMap<String, InvestUser>,

    #[serde(skip)]
    chance: f64,

    #[serde(skip)]
    starting: usize,

    #[serde(skip)]
    line_value: usize,

    #[serde(skip)]
    emote_value: Vec<[usize; 2]>,
}

impl Default for InvestState {
    fn default() -> Self {
        Self {
            starting: 0,
            line_value: 5,
            chance: 1.0 / 2.0,
            emote_value: vec![[0, 0]],

            total: 0,
            state: Default::default(),
        }
    }
}

impl Drop for InvestState {
    fn drop(&mut self) {
        debug!("saving InvestState to {}", INVEST_STORE);
        self.save();
    }
}

enum IncrementType {
    Line,
    Emote(usize), // count (for decay)
}

impl InvestState {
    #[cfg(not(test))]
    pub fn load(config: &config::Config) -> Self {
        use std::fs;
        debug!("loading InvestState from: {}", INVEST_STORE);
        let s = fs::read_to_string(INVEST_STORE)
            .or_else(|_| {
                debug!("loading default Invest");
                serde_json::to_string_pretty(&InvestState::default())
            }).expect("to get json");
        let mut this: Self = serde_json::from_str(&s).expect("to deserialize struct");
        this.starting = config.invest.starting;
        this.line_value = config.invest.line_value;
        this.chance = config.invest.chance;
        this.emote_value = config.invest.kappas.to_vec();
        this
    }

    #[cfg(test)]
    pub fn load(_config: &config::Config) -> Self {
        InvestState::default()
    }

    pub fn save(&self) {
        #[cfg(not(test))]
        {
            use std::fs;
            let f = fs::File::create(INVEST_STORE).expect("to create file");
            serde_json::to_writer(&f, &self).expect("to serialize struct");
            trace!("saving Invest to {}", INVEST_STORE)
        }
    }

    pub fn get(&mut self, id: &str) -> &InvestUser {
        self.state
            .entry(id.into())
            .or_insert_with(|| InvestUser::new(id))
    }

    pub fn give(&mut self, id: &str, credits: Credit) -> Credit {
        self.state
            .entry(id.into())
            .and_modify(|c| {
                c.current += credits;
                c.total += credits;
                if c.current > c.max {
                    c.max = c.current;
                }
            }).or_insert_with(|| {
                let mut user = InvestUser::new(id);
                user.current = credits;
                user.max = credits;
                user.total = credits;
                user
            });

        self.save();
        let credits = self.state[id].current;
        trace!("setting {}'s credits to {}", id, credits);
        credits
    }

    pub fn take(&mut self, id: &str, credits: Credit) -> Credit {
        self.state
            .entry(id.into())
            .and_modify(|c| c.current -= credits)
            .or_insert_with(|| {
                let mut user = InvestUser::new(id);
                user.current = credits;
                user
            });

        self.save();
        let credits = self.state[id].current;
        trace!("setting {}'s credits to {}", id, credits);
        credits
    }

    pub fn increment(&mut self, id: &str, ty: &IncrementType) -> Credit {
        let value = || -> usize {
            match ty {
                IncrementType::Line => self.line_value,
                IncrementType::Emote(e) => {
                    for a in &self.emote_value {
                        if *e <= a[1] {
                            return a[0];
                        }
                    }
                    0
                }
            }
        }();

        self.give(id, value)
    }

    fn invest_success(&mut self, id: &str, have: Credit, want: Credit) -> InvestResult {
        self.state.entry(id.into()).and_modify(|c| {
            c.current += want;
            c.total += want;
            c.invest.0 += 1;
            if c.current > c.max {
                c.max = c.current
            }
        });

        let amount = self.state[id].current;
        debug!("donation was successful: {}, {} -> {}", id, have, amount);
        Ok(Donation::Success {
            old: have,
            new: amount,
        })
    }

    fn invest_failure(&mut self, id: &str, have: Credit, want: Credit) -> InvestResult {
        self.state.entry(id.into()).and_modify(|c| {
            if let Some(v) = c.current.checked_sub(want) {
                c.current = v
            } else {
                c.current = 0;
            };
            c.invest.1 += 1;
        });

        let amount = self.state[id].current;
        self.total += want;

        debug!("donation was a failure: {}, {} -> {}", id, have, amount);
        Ok(Donation::Failure {
            old: have,
            new: amount,
        })
    }

    fn try_donation(&mut self, id: &str, have: usize, want: usize) -> InvestResult {
        if have == 0 || want > have {
            Err(InvestError::NotEnoughCredits { have, want })?
        }

        let res = if thread_rng().gen_bool(self.chance) {
            self.invest_failure(id, have, want)
        } else {
            self.invest_success(id, have, want)
        };

        self.save();
        res
    }

    pub fn invest(&mut self, id: &str, want: Credit) -> InvestResult {
        if let Some(have) = self.get_credits_for(id) {
            self.try_donation(id, have, want)
        } else {
            Err(InvestError::NotEnoughCredits { have: 0, want })
        }
    }

    pub fn get_credits_for(&self, id: &str) -> Option<Credit> {
        self.state.get(id).and_then(|c| Some(c.current))
    }

    pub fn to_sorted(&self) -> Vec<(String, Credit)> {
        let mut sorted = self
            .state
            .iter()
            .map(|(k, v)| (k.to_string(), v.current))
            .collect::<Vec<_>>();
        sorted.sort_by(|l, r| r.1.cmp(&l.1));
        // TODO: should also sort it by name if equal credits
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
        let _module = Invest::new(&env.bot, &env.config);

        env.push_privmsg("!invest");
        env.step();

        assert_eq!(
            env.pop_env().unwrap().data,
            "@test: thats not a number I understand"
        );

        warn!("{:#?}", _module.inner.read().state);

        env.push_privmsg("!invest 10");
        env.step();

        assert_eq!(
            env.pop_env().unwrap().data,
            "@test: you don't have enough. you have 0, but you want to invest 10 credits"
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
        let _module = Invest::new(&env.bot, &env.config);

        env.push_privmsg("!give");
        env.step();

        assert_eq!(
            env.pop_env().unwrap().data,
            "@test: who do you want to give credits to?"
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
        let _module = Invest::new(&env.bot, &env.config);

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
    fn test_stats_commande() {
        let env = Environment::new();
        let module = Invest::new(&env.bot, &env.config);

        init_logger();

        env.push_privmsg("!stats");
        env.step();
        assert_eq!(env.pop_env().unwrap().data, "@test: you've reached a max of 0 credits, out of 0 total credits with 0 successes and 0 failures (0.00%).. and I've 'collected' 0 credits");
        assert_eq!(env.pop_env(), None);

        fn failure(m: &Invest) {
            m.inner.write().state.chance = 1.0;
        }

        fn success(m: &Invest) {
            m.inner.write().state.chance = 0.0;
        }

        fn give(m: &Invest, n: usize) {
            m.inner.write().state.give("1004", n);
        }

        failure(&module);
        give(&module, 1000);

        env.push_privmsg("!stats");
        env.step();
        assert_eq!(env.pop_env().unwrap().data, "@test: you've reached a max of 1,000 credits, out of 1,000 total credits with 0 successes and 0 failures (0.00%).. and I've 'collected' 0 credits");

        env.push_privmsg("!invest 500");
        env.step();
        env.drain_envs();

        env.push_privmsg("!stats");
        env.step();
        assert_eq!(env.pop_env().unwrap().data, "@test: you've reached a max of 1,000 credits, out of 1,000 total credits with 0 successes and 1 failures (0.00%).. and I've 'collected' 500 credits");

        env.push_privmsg("!invest 300");
        env.step();
        env.drain_envs();

        env.push_privmsg("!stats");
        env.step();
        assert_eq!(env.pop_env().unwrap().data, "@test: you've reached a max of 1,000 credits, out of 1,000 total credits with 0 successes and 2 failures (0.00%).. and I've 'collected' 800 credits");

        success(&module);

        env.push_privmsg("!invest 200");
        env.step();
        env.drain_envs();

        env.push_privmsg("!stats");
        env.step();
        assert_eq!(env.pop_env().unwrap().data, "@test: you've reached a max of 1,000 credits, out of 1,200 total credits with 1 successes and 2 failures (200.00%).. and I've 'collected' 800 credits");

        env.push_privmsg("!invest all");
        env.step();
        env.push_privmsg("!invest all");
        env.step();
        env.drain_envs();

        env.push_privmsg("!stats");
        env.step();
        env.drain_envs_warn_log();

        failure(&module);
        env.push_privmsg("!invest all");
        env.step();
        env.drain_envs();

        env.push_privmsg("!stats");
        env.step();
        env.drain_envs_warn_log();

        //env.drain_envs_warn_log();

        // TODO write more tests for this
    }

    #[test]
    fn test_invest() {
        let mut ch = InvestState::default();

        assert_eq!(
            ch.invest("test", 10),
            Err(InvestError::NotEnoughCredits { have: 0, want: 10 })
        ); // not seen before. so zero credits

        assert_eq!(ch.increment("foo", &IncrementType::Line), 5); // starts at 0, so +5
        assert_eq!(ch.increment("foo", &IncrementType::Line), 10); // then +5

        assert_eq!(
            ch.invest("test", 10),
            Err(InvestError::NotEnoughCredits { have: 0, want: 10 })
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
