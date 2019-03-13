use crate::prelude::*;

mod game;
use self::game::*;

use std::str;
use std::time::{Duration, Instant};

use hashbrown::HashMap;
use rand::prelude::*;

pub const NAME: &str = "Invest";

submit! {
    template::Response("invest_no_credits", "you don't have any credits.");
    template::Response("invest_zero_number", "zero what?");
    template::Response("invest_success", "success! you went from ${old} to ${new}");
    template::Response("invest_success_delta", "success! you went from ${old} to ${new} (+${delta})");
    template::Response("invest_failure", "failure! you went from ${old} to ${new} (-${delta}). try again in a minute");
    template::Response("invest_requires_credits", "you don't have enough. you have ${have} but you want to invest ${want}.");
    template::Response("invest_no_target", "who do you want to give credits to?");
    template::Response("invest_send_to_self", "what are you doing?");
    template::Response("invest_send_to_bot", "I don't want any credits.");
    template::Response("invest_unknown_target", "I don't know who that is.");
    template::Response("invest_not_enough_credits", "you only have ${current} credits.");
    template::Response("invest_send_success", "they now have ${them} credits and you're down to ${you} credits.");
    template::Response("invest_check_credits", "you have ${credits} credits.");
    template::Response("invest_leaderboard", "(#${n}) ${display}: ${credits}");
    template::Response("invest_stats", "you've reached a max of ${max} credits, out of ${total} total credits with ${success} successes and ${failure} failures. and I've 'collected' ${overall_total} credits from all of the failures.");
}

pub struct Invest {
    config: config::Invest,
    limit: HashMap<i64, Instant>,
    last: Instant,
    map: CommandMap<Invest>,
}

impl Module for Invest {
    fn command(&mut self, req: &Request) -> Option<Response> {
        let map = self.map.clone();
        map.dispatch(self, req)
    }

    fn passive(&mut self, msg: &irc::Message) -> Option<Response> {
        if msg.command == "PRIVMSG" {
            return self.on_message(msg);
        }
        None
    }

    fn tick(&mut self, dt: Instant) -> Option<Response> {
        if dt - self.last >= Duration::from_secs(self.config.interval as u64) {
            InvestGame::increment_all_active(&get_connection(), 1);
            self.last = dt
        }
        None
    }
}

impl Invest {
    pub fn create() -> Result<Self, ModuleError> {
        ensure_table(InvestGame::ensure_table);

        let map = CommandMap::create(
            NAME,
            &[
                ("!invest", Self::invest_command),
                ("!give", Self::give_command),
                ("!check", Self::check_command),
                ("!top5", Self::top5_command),
                ("!top", Self::top5_command),
                ("!stats", Self::stats_command),
            ],
        )?;

        let config = Config::load();
        Ok(Self {
            config: config.invest.clone(),
            limit: HashMap::new(),
            last: Instant::now(),
            map,
        })
    }

    fn invest_command(&mut self, req: &Request) -> Option<Response> {
        let id = req.sender();
        if self.check_rate_limit(id) {
            // they've been rate limited
            return None;
        }

        let user = InvestGame::find(id);
        let user = match user.as_ref() {
            Some(user) if user.current > 0 => user,
            None | _ => return reply_template!("invest_no_credits"),
        };
        let (ty, num) = match Self::get_credits_from_args(user.current, req.args_iter()) {
            Some((_, num)) if num == 0 => return reply_template!("invest_zero_number"),
            Some((ty, num)) => (ty, num),
            None => return reply_template!("misc_invalid_number"),
        };

        match InvestGame::invest(self.config.chance, id, num) {
            Ok(Investment::Success { old, new }) => match ty {
                NumType::Random => reply_template!(
                    "invest_success_delta",
                    ("old", &old.commas()),            //
                    ("new", &new.commas()),            //
                    ("deltra", &(new - old).commas()), //
                ),
                _ => reply_template!(
                    "invest_success",
                    ("old", &old.commas()),
                    ("new", &new.commas())
                ),
            },
            Ok(Investment::Failure { old, new }) => {
                self.rate_limit(id);
                reply_template!(
                    "invest_failure",
                    ("old", &old.commas()),           //
                    ("new", &new.commas()),           //
                    ("delta", &(old - new).commas()), //
                )
            }
            Err(InvestError::NotEnoughCredits { have, want }) => reply_template!(
                "invest_requires_credits",
                ("have", &have.commas()), //
                ("want", &want.commas()), //
            ),
            Err(_) => {
                // what to do here?
                None
            }
        }
    }

    fn give_command(&mut self, req: &Request) -> Option<Response> {
        let conn = get_connection();

        let id = req.sender();
        let sender = UserStore::get_user_by_id(&conn, id)?;
        let user = InvestGame::find(id)?;
        if user.current == 0 {
            return reply_template!("invest_no_credits");
        }

        let mut args = req.args_iter();
        let mut target = match args.next() {
            Some(target) => target,
            None => return reply_template!("invest_no_target"),
        };

        if target.starts_with('@') {
            target = &target[1..]
        }

        if target.eq_ignore_ascii_case(&sender.display) {
            return reply_template!("invest_send_to_self");
        }

        let me = UserStore::get_bot(&conn).expect("get bot user info");
        if target.eq_ignore_ascii_case(&me.display) {
            return reply_template!("invest_send_to_bot");
        }

        let tid = match UserStore::get_user_by_name(&conn, &target) {
            Some(user) => user,
            None => return reply_template!("invest_unknown_target"),
        };

        let (_ty, num) = match Self::get_credits_from_args(user.current, args) {
            Some((_, num)) if num == 0 => return reply_template!("invest_zero_number"),
            Some((ty, num)) => (ty, num),
            None => return reply_template!("misc_invalid_number"),
        };

        if num > user.current {
            return reply_template!(
                "invest_not_enough_credits",
                ("current", &user.current.commas())
            );
        }

        let (them, you) = {
            let c = InvestGame::give(tid.userid, num).expect("give credits");
            let d = InvestGame::take(user.id, num).expect("take credits");
            (c, d)
        };

        reply_template!(
            "invest_send_success",
            ("them", &them.commas()), //
            ("you", &you.commas()),   //
        )
    }

    fn check_command(&mut self, req: &Request) -> Option<Response> {
        match InvestGame::find(req.sender()).unwrap().current {
            credits if credits > 0 => {
                reply_template!("invest_check_credits", ("credits", &credits.commas()))
            }
            _ => reply_template!("invest_no_credits"),
        }
    }

    fn top5_command(&mut self, req: &Request) -> Option<Response> {
        let n = req
            .args_iter()
            .next()
            .and_then(|s| s.parse::<u16>().ok())
            .unwrap_or_else(|| 5);

        // sanity checks because I'm sure someone will do it
        // clamp it between 5 and 10
        let n = match n {
            n if n > 10 => 10,
            n if n < 5 => 5,
            n => n,
        };

        let conn = get_connection();
        let list = InvestGame::get_top_n(&conn, n as i16)
            .into_iter()
            .enumerate()
            .map(|(i, iu)| {
                let user = UserStore::get_user_by_id(&conn, iu.id).expect("user to exist");
                let args = template::TemplateArgs::new()
                    .with("n", &(i + 1))
                    .with("display", &user.display)
                    .with("credits", &iu.current.commas())
                    .build();
                template::lookup("invest_leaderboard", &args).unwrap()
            });

        reply!(list.collect::<Vec<_>>().join(", "))
    }

    fn stats_command(&mut self, req: &Request) -> Option<Response> {
        let id = req.sender();
        let (user, total) = InvestGame::stats_for(id);

        reply_template!(
            "invest_stats",
            ("max", &user.max.commas()),          //
            ("total", &user.total.commas()),      //
            ("success", &user.invest.0.commas()), //
            ("failure", &user.invest.1.commas()), //
            ("overall_total", &total.commas()),   //
        )
    }

    fn on_message(&self, msg: &irc::Message) -> Option<Response> {
        if msg.expect_data().starts_with('!') || msg.expect_data().starts_with('@') {
            return None;
        }

        let id = msg.tags.get_userid()?;
        InvestGame::give(id, self.config.line_value);
        InvestGame::set_active(id);

        fn parse_decay(s: &str) -> Vec<(usize, usize)> {
            s.split(',').fold(vec![], |mut list, s| {
                let mut s = s.split(':').filter_map(|s| s.parse::<usize>().ok());
                if let (Some(l), Some(r)) = (s.next(), s.next()) {
                    list.push((l, r))
                };
                list
            })
        }

        let kappas = msg.tags.get_kappas()?;
        let len = kappas.len();
        for (points, decay) in parse_decay(&self.config.kappas) {
            if len <= decay {
                InvestGame::give(id, points);
                return None;
            }
        }
        None
    }

    fn get_credits_from_args<'a>(
        credits: Credit,
        mut parts: impl Iterator<Item = &'a str>,
    ) -> Option<(NumType, Credit)> {
        let data = parts.next()?.trim();
        let ty = parse_number_or_context(&data)?;
        Some((
            ty,
            match ty {
                NumType::Num(n) => n,
                NumType::All => credits,
                NumType::Half => credits / 2,
                NumType::Random => thread_rng().gen_range(1, credits),
            },
        ))
    }

    fn rate_limit(&mut self, id: i64) {
        self.limit.insert(id, Instant::now());
    }

    fn check_rate_limit(&self, id: i64) -> bool {
        if let Some(t) = self.limit.get(&id) {
            if Instant::now() - *t < Duration::from_secs(60) {
                return true;
            }
        }
        false
    }
}

#[derive(Copy, Clone)]
enum NumType {
    Num(Credit),
    All,
    Half,
    Random,
}

fn parse_number_or_context(data: &str) -> Option<NumType> {
    const CONTEXTS: [&str; 3] = ["all", "half", "random"];

    let num = data
        .chars()
        .take_while(char::is_ascii_digit)
        .collect::<String>();

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
        let db = database::get_connection();
        {
            let mut invest = Invest::create().unwrap();
            invest.config.chance = 0.0;
            let mut env = Environment::new(&db, &mut invest);

            env.push("!invest 10");
            env.step();
            assert_eq!(env.pop().unwrap(), "@test: you don't have any credits.");

            InvestGame::give(env.get_user_id(), 100);
            env.push("!invest 10");
            env.step();
            assert_eq!(
                env.pop().unwrap(),
                "@test: success! you went from 100 to 110"
            );
        };

        let mut invest = Invest::create().unwrap();
        invest.config.chance = 1.0;
        let mut env = Environment::new(&db, &mut invest);

        env.push("!invest 10");
        env.step();
        assert_eq!(
            env.pop().unwrap(),
            "@test: failure! you went from 110 to 100 (-10). try again in a minute"
        );
        env.push("!invest 10");
        env.step_wait(false);
        assert_eq!(env.pop(), None);
    }

    #[test]
    fn give_command() {
        let db = database::get_connection();
        let mut invest = Invest::create().unwrap();
        let mut env = Environment::new(&db, &mut invest);

        env.push("!give foo 10");
        env.step();
        assert_eq!(env.pop().unwrap(), "@test: you don't have any credits.");

        InvestGame::give(env.get_user_id(), 100);
        env.push("!give");
        env.step();
        assert_eq!(
            env.pop().unwrap(),
            "@test: who do you want to give credits to?"
        );

        env.push("!give test");
        env.step();
        assert_eq!(env.pop().unwrap(), "@test: what are you doing?");

        env.push("!give shaken_bot");
        env.step();
        assert_eq!(env.pop().unwrap(), "@test: I don't want any credits.");

        env.push("!give foo");
        env.step();
        assert_eq!(env.pop().unwrap(), "@test: I don't know who that is.");

        let _user = make_test_user(&db, "foo", 1001);

        env.push("!give foo");
        env.step();
        assert_eq!(env.pop().unwrap(), "@test: thats not a number I understand");

        env.push("!give foo 101");
        env.step();
        assert_eq!(env.pop().unwrap(), "@test: you only have 100 credits.");

        env.push("!give foo 50");
        env.step();
        assert_eq!(
            env.pop().unwrap(),
            "@test: they now have 50 credits and you're down to 50 credits."
        );
    }

    #[test]
    fn check_command() {
        let db = database::get_connection();
        let mut invest = Invest::create().unwrap();
        let mut env = Environment::new(&db, &mut invest);

        env.push("!check");
        env.step();
        assert_eq!(env.pop().unwrap(), "@test: you don't have any credits.");

        InvestGame::give(env.get_user_id(), 100);

        env.push("!check");
        env.step();
        assert_eq!(env.pop().unwrap(), "@test: you have 100 credits.");
    }

    #[test]
    fn top5_command() {
        use rand::{distributions::Alphanumeric, thread_rng, Rng};

        let db = database::get_connection();
        let mut invest = Invest::create().unwrap();
        let mut env = Environment::new(&db, &mut invest);

        for n in 1001..1012 {
            let name = thread_rng()
                .sample_iter(&Alphanumeric)
                .take(10)
                .collect::<String>();
            let _u = make_test_user(&db, &name, n as i64);
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
}
