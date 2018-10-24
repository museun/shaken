pub(crate) mod game;
use self::game::*;

use parking_lot::Mutex;
use rand::prelude::*;

use std::collections::HashMap;
use std::str;
use std::time::{Duration, Instant};

use crate::config;
use crate::database::{ensure_table, get_connection};
use crate::irc::Message;
use crate::util::*;
use crate::*;

impl Default for Invest {
    fn default() -> Self {
        Self::new()
    }
}

pub struct Invest {
    config: config::Invest,
    limit: Mutex<HashMap<i64, Instant>>,
    commands: Vec<Command<Invest>>,
    _every: Every,
}

impl Module for Invest {
    fn command(&self, req: &Request) -> Option<Response> {
        dispatch_commands!(&self, &req)
    }

    fn passive(&self, msg: &Message) -> Option<Response> {
        // for now
        match &msg.command[..] {
            "PRIVMSG" => self.on_message(msg),
            _ => None,
        }
    }
}

impl Invest {
    pub fn new() -> Self {
        ensure_table(InvestGame::ensure_table);

        let (_, every) = every!(
            |_, _| InvestGame::increment_all_active(&get_connection(), 1),
            (),
            60 * 1000
        );

        let config = Config::load();
        Self {
            config: config.invest.clone(),
            limit: Mutex::new(HashMap::new()),
            commands: command_list!(
                ("!invest", Self::invest_command),
                ("!give", Self::give_command),
                ("!check", Self::check_command),
                ("!top5", Self::top5_command),
                ("!top", Self::top5_command),
                ("!stats", Self::stats_command),
            ),
            _every: every, // store this so the update loop stays alive
        }
    }

    fn invest_command(&self, req: &Request) -> Option<Response> {
        let id = req.sender();
        if self.check_rate_limit(id) {
            // they've been rate limited
            return None;
        }

        let user = match InvestGame::find(id) {
            // could use some guard patterns, but the borrowck isn't there yet
            Some(user) => {
                if user.current > 0 {
                    user
                } else {
                    return reply!("you don't have any credits.");
                }
            }
            None => return reply!("you don't have any credits."),
        };
        let (ty, num) = match Self::get_credits_from_args(user.current, req.args_iter()) {
            Some((_, num)) if num == 0 => return reply!("zero what?"),
            Some((ty, num)) => (ty, num),
            None => return reply!("thats not a number I understand"),
        };

        match InvestGame::invest(self.config.chance, id, num) {
            Ok(Investment::Success { old, new }) => match ty {
                NumType::Random => reply!(
                    "success! you went from {} to {} (+{})",
                    old.commas(),
                    new.commas(),
                    (new - old).commas()
                ),
                _ => reply!(
                    "success! you went from {} to {}",
                    old.commas(),
                    new.commas(),
                ),
            },
            Ok(Investment::Failure { old, new }) => {
                self.rate_limit(id);
                reply!(
                    "failure! you went from {} to {} (-{}). try again in a minute",
                    old.commas(),
                    new.commas(),
                    (old - new).commas(),
                )
            }
            Err(InvestError::NotEnoughCredits { have, want }) => reply!(
                "you don't have enough. you have {} but you want to invest {}.",
                have.commas(),
                want.commas()
            ),
            Err(_) => {
                // what to do here?
                None
            }
        }
    }

    fn give_command(&self, req: &Request) -> Option<Response> {
        let conn = get_connection();

        let id = req.sender();
        let sender = UserStore::get_user_by_id(&conn, id)?;
        let user = InvestGame::find(id)?;
        if user.current == 0 {
            return reply!("you don't have any credits");
        }

        let mut args = req.args_iter();
        let mut target = match args.next() {
            Some(target) => target,
            None => {
                return reply!("who do you want to give credits to?");
            }
        };

        if target.starts_with('@') {
            target = &target[1..]
        }

        if target.eq_ignore_ascii_case(&sender.display) {
            return reply!("what are you doing?");
        }

        let me =
            UserStore::get_bot(&conn, &Config::load().twitch.name).expect("to get bot user info");
        if target.eq_ignore_ascii_case(&me.display) {
            return reply!("I don't want any credits.");
        }

        let tid = match UserStore::get_user_by_name(&conn, &target) {
            Some(user) => user,
            None => {
                return reply!("I don't know who that is.");
            }
        };

        let (_ty, num) = match Self::get_credits_from_args(user.current, args) {
            Some((_, num)) if num == 0 => return reply!("zero what?"),
            Some((ty, num)) => (ty, num),
            None => return reply!("thats not a number I understand"),
        };

        if num > user.current {
            return reply!("you only have {} credits", user.current.commas());
        }

        let (c, d) = {
            let c = InvestGame::give(tid.userid, num).expect("give credits");
            let d = InvestGame::take(user.id, num).expect("take credits");
            (c, d)
        };

        reply!(
            "they now have {} credits and you're down to {} credits",
            c.commas(),
            d.commas()
        )
    }

    fn check_command(&self, req: &Request) -> Option<Response> {
        match InvestGame::find(req.sender()).unwrap().current {
            credits if credits > 0 => reply!("you have {} credits", credits.commas()),
            _ => reply!("you don't have any credits"),
        }
    }

    fn top5_command(&self, req: &Request) -> Option<Response> {
        let mut n = req
            .args_iter()
            .next()
            .and_then(|s| s.parse::<u16>().ok())
            .or_else(|| Some(5))
            .unwrap();

        // sanity checks because I'm sure someone will do it
        // clamp it between 5 and 10
        if n > 10 {
            n = 10
        }

        if n < 5 {
            n = 5
        }

        let conn = get_connection();
        let list = InvestGame::get_top_n(n as i16, &SortBy::Current)
            .iter()
            .enumerate()
            .map(|(i, iu)| {
                let user = UserStore::get_user_by_id(&conn, iu.id).expect("user to exist");
                format!("(#{}) {}: {}", i + 1, &user.display, iu.current.commas())
            })
            .collect::<Vec<_>>(); // this collect is needed

        reply!("{}", crate::util::join_with(list.iter(), ", "))
    }

    fn stats_command(&self, req: &Request) -> Option<Response> {
        let id = req.sender();
        let (user, total) = InvestGame::stats_for(id);

        reply!(
            "you've reached a max of {} credits, out of {} total credits with {} successes and {} \
             failures. and I've 'collected' {} credits from all of the failures.",
            user.max.commas(),
            user.total.commas(),
            user.invest.0.commas(),
            user.invest.1.commas(),
            total.commas()
        )
    }

    fn on_message(&self, msg: &Message) -> Option<Response> {
        if msg.data.starts_with('!') || msg.data.starts_with('@') {
            return None;
        }

        let id = msg.tags.get_userid()?;
        InvestGame::give(id, self.config.line_value);
        InvestGame::set_active(id);

        if let Some(kappas) = msg.tags.get_kappas() {
            let len = kappas.len();
            for a in &self.config.kappas {
                if len <= a[1] {
                    InvestGame::give(id, a[0]);
                    return None;
                }
            }
        }

        None
    }

    fn get_credits_from_args<'a>(
        credits: Credit,
        mut parts: impl Iterator<Item = &'a str>,
    ) -> Option<(NumType, Credit)> {
        let data = parts.next()?.trim();
        Some(match parse_number_or_context(&data)? {
            ty @ NumType::Num(_) => {
                let n = match &ty {
                    // TODO: why is this needed?
                    NumType::Num(n) => *n,
                    _ => unreachable!(),
                };
                (ty, n)
            }
            ty @ NumType::All => (ty, credits),
            ty @ NumType::Half => (ty, credits / 2),
            ty @ NumType::Random => (ty, thread_rng().gen_range(1, credits)),
        })
    }

    fn rate_limit(&self, id: i64) {
        self.limit.lock().insert(id, Instant::now());
    }

    fn check_rate_limit(&self, id: i64) -> bool {
        if let Some(t) = self.limit.lock().get(&id) {
            if Instant::now() - *t < Duration::from_secs(60) {
                return true;
            }
        }
        false
    }
}

enum NumType {
    Num(Credit),
    All,
    Half,
    Random,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::*;

    #[test]
    fn invest_command() {
        let mut invest = Invest::new();
        invest.config.chance = 0.0;

        let mut env = Environment::new();
        env.add(&invest);
        // sequencing..
        InvestGame::ensure_table(env.get_db_conn());

        env.push("!invest 10");
        env.step();
        assert_eq!(env.pop(), Some("@test: you don't have any credits.".into()));

        InvestGame::give(env.get_user_id(), 100);
        env.push("!invest 10");
        env.step();
        assert_eq!(
            env.pop(),
            Some("@test: success! you went from 100 to 110".into())
        );

        // borrowing is fun
        let mut invest = Invest::new();
        invest.config.chance = 1.0;

        let mut env = Environment::new();
        env.add(&invest);

        env.push("!invest 10");
        env.step();
        assert_eq!(
            env.pop(),
            Some("@test: failure! you went from 110 to 100 (-10). try again in a minute".into())
        );
        env.push("!invest 10");
        env.step();
        assert_eq!(env.pop(), None);
    }

    #[test]
    fn give_command() {
        // to hold on to it.
        let conn = get_connection();

        let invest = Invest::new();
        let mut env = Environment::new();
        env.add(&invest);

        InvestGame::ensure_table(env.get_db_conn());

        env.push("!give foo 10");
        env.step();
        assert_eq!(env.pop(), Some("@test: you don't have any credits".into()));

        InvestGame::give(env.get_user_id(), 100);
        env.push("!give");
        env.step();
        assert_eq!(
            env.pop(),
            Some("@test: who do you want to give credits to?".into())
        );

        env.push("!give test");
        env.step();
        assert_eq!(env.pop(), Some("@test: what are you doing?".into()));

        env.push("!give shaken_bot");
        env.step();
        assert_eq!(env.pop(), Some("@test: I don't want any credits.".into()));

        env.push("!give foo");
        env.step();
        assert_eq!(env.pop(), Some("@test: I don't know who that is.".into()));

        let _user = make_test_user(&conn, "foo", 1001);

        env.push("!give foo");
        env.step();
        assert_eq!(
            env.pop(),
            Some("@test: thats not a number I understand".into())
        );

        env.push("!give foo 101");
        env.step();
        assert_eq!(env.pop(), Some("@test: you only have 100 credits".into()));

        env.push("!give foo 50");
        env.step();
        assert_eq!(
            env.pop(),
            Some("@test: they now have 50 credits and you're down to 50 credits".into())
        );
    }

    #[test]
    fn check_command() {
        // to hold on to it.
        let conn = get_connection();

        let invest = Invest::new();
        let mut env = Environment::new();
        env.add(&invest);

        InvestGame::ensure_table(env.get_db_conn());

        env.push("!check");
        env.step();
        assert_eq!(env.pop(), Some("@test: you don't have any credits".into()));

        InvestGame::give(env.get_user_id(), 100);

        env.push("!check");
        env.step();
        assert_eq!(env.pop(), Some("@test: you have 100 credits".into()));
    }

    #[test]
    fn top5_command() {
        // to hold on to it.
        let conn = get_connection();

        let invest = Invest::new();
        let mut env = Environment::new();
        env.add(&invest);

        InvestGame::ensure_table(env.get_db_conn());

        use rand::{distributions::Alphanumeric, thread_rng, Rng};

        for n in 1001..1012 {
            let name = thread_rng()
                .sample_iter(&Alphanumeric)
                .take(10)
                .collect::<String>();
            let _u = make_test_user(&conn, &name, n as i64);
            let r = thread_rng().gen::<u16>();
            InvestGame::give(n, r as usize);
        }

        env.push("!top5");
        env.step();
        assert!(env.pop().is_some());

        env.push("!top 5");
        env.step();
        assert!(env.pop().is_some());

        env.push("!top 10");
        env.step();
        assert!(env.pop().is_some());

        env.push("!top 100");
        env.step();
        assert!(env.pop().is_some());

        env.push("!top 0");
        env.step();
        assert!(env.pop().is_some());
    }

    #[test]
    #[ignore] // this requires too much set up. its literally just formatting a string
    fn stats_command() {
        // to hold on to it.
        let conn = get_connection();

        let invest = Invest::new();
        let mut env = Environment::new();
        env.add(&invest);

        InvestGame::ensure_table(env.get_db_conn());

        env.push("!stats");
        env.step();
    }

    #[test]
    #[ignore] // TODO test this
    fn on_message() {}
}