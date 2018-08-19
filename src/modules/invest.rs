use rand::prelude::*;
use rusqlite::Connection;

use std::collections::HashMap;
use std::str;
use std::time::{Duration, Instant};

use parking_lot::Mutex;

use crate::*;
use crate::{config, database::get_connection, irc::Message, twitch::TwitchClient, util::*};

impl Default for Invest {
    fn default() -> Self {
        Self::new()
    }
}

pub struct Invest {
    config: config::Invest,
    twitch: TwitchClient,
    limit: Mutex<HashMap<i64, Instant>>,
    commands: Vec<Command<Invest>>,
    every: Every,
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
        InvestGame::ensure_table(&get_connection());

        let (_, every) = every!(
            |_, _| InvestGame::increment_all_active(&get_connection(), 1),
            (),
            60 * 1000
        );

        let config = Config::load();
        Self {
            config: config.invest.clone(),
            twitch: TwitchClient::new(),
            limit: Mutex::new(HashMap::new()),
            commands: command_list!(
                ("!invest", Self::invest_command),
                ("!give", Self::give_command),
                ("!check", Self::check_command),
                ("!top5", Self::top5_command),
                ("!top", Self::top5_command),
                ("!stats", Self::stats_command),
            ),
            every, // store this so the update loop stays alive
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
        let num = match Self::get_credits_from_args(user.current, req.args_iter()) {
            Some(num) if num == 0 => return reply!("zero what?"),
            Some(num) => num,
            None => return reply!("thats not a number I understand"),
        };

        match InvestGame::invest(self.config.chance, id, num) {
            Ok(Investment::Success { old, new }) => reply!(
                "success! you went from {} to {} (+{})",
                old.commas(),
                new.commas(),
                (new - old).commas()
            ),
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

        let num = match Self::get_credits_from_args(user.current, args) {
            Some(num) if num == 0 => return reply!("zero what?"),
            Some(num) => num,
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
            }).collect::<Vec<_>>(); // this collect is needed

        reply!("{}", crate::util::join_with(list.iter(), ", "))
    }

    fn stats_command(&self, req: &Request) -> Option<Response> {
        let id = req.sender();
        let (user, total) = InvestGame::stats_for(id);

        reply!("you've reached a max of {} credits, out of {} total credits with {} successes and {} failures. and I've 'collected' {} credits from all of the failures.",
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
    ) -> Option<Credit> {
        let data = parts.next()?.trim();
        Some(match parse_number_or_context(&data)? {
            NumType::Num(num) => num,
            NumType::All => credits,
            NumType::Half => credits / 2,
            NumType::Random => thread_rng().gen_range(1, credits),
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
    use testing::*;

    fn dump(hd: &str, conn: &Connection) {
        macro_rules! un {
            ($e:expr, $n:expr) => {
                $e.get::<_, i64>($n) as usize
            };
        }

        let mut stmt = conn
            .prepare("SELECT * FROM Invest")
            .expect("valid sql in ensure");

        let mut iter = stmt
            .query_map(&[], |row| InvestUser {
                id: row.get(0),
                max: un!(row, 1),
                current: un!(row, 2),
                total: un!(row, 3),
                invest: (un!(row, 4), un!(row, 5)),
                active: row.get(6),
            }).map_err(|e| error!("{}", e))
            .expect("to get rows");

        trace!("trying to dump database");

        while let Some(Ok(row)) = iter.next() {
            warn!("{} >> {:?}", hd, row)
        }
    }

    #[test]
    fn invest_command() {
        let mut invest = Invest::new();
        invest.config.chance = 0.0;

        let mut env = Environment::new();
        env.add(&invest);

        env.push("!invest 10");
        env.step();
        assert_eq!(env.pop(), Some("@test: you don't have any credits.".into()));

        InvestGame::give(env.get_user_id(), 100);
        env.push("!invest 10");
        env.step();
        assert_eq!(
            env.pop(),
            Some("@test: success! you went from 100 to 110 (+10)".into())
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

        use rand::distributions::Alphanumeric;
        use rand::{thread_rng, Rng};

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

        env.push("!stats");
        env.step();
    }

    #[test]
    #[ignore] // TODO test this
    fn on_message() {}
}

const INVEST_TABLE: &str = r#"
BEGIN;

CREATE TABLE IF NOT EXISTS InvestStats (
    ID INTEGER PRIMARY KEY NOT NULL UNIQUE,
    Total INTEGER NOT NULL
);

INSERT OR IGNORE INTO InvestStats (ID, Total) VALUES(0, 0);

CREATE TABLE IF NOT EXISTS Invest (
    ID      INTEGER PRIMARY KEY NOT NULL UNIQUE, -- twitch ID
    Max     INTEGER NOT NULL,                    -- highest credits held
    Current INTEGER NOT NULL,                    -- current credit balance
    Total   INTEGER NOT NULL,                    -- total credits earned
    Success INTEGER NOT NULL,                    -- number of successes
    Failure INTEGER NOT NULL,                    -- number of failures
    Active  INTEGER NOT NULL                     -- whether they'll get idle points
    -- FOREIGN KEY(ID) REFERENCES Users(ID) -- maybe add this constraint later
);

COMMIT;
"#;

pub type Credit = usize;
pub type InvestResult<T> = std::result::Result<T, InvestError>;

#[derive(Debug, PartialEq)]
pub enum InvestError {
    NotEnoughCredits { have: Credit, want: Credit },
    CannotInsert { id: i64 },
    UserNotFound { id: i64 },
    NoRowsFound { id: i64 },
}

#[derive(Debug, PartialEq)]
pub enum Investment {
    Success { old: Credit, new: Credit },
    Failure { old: Credit, new: Credit },
}

pub enum IncrementType {
    Line,
    Emote(usize), // count (for decay)
}

pub enum SortBy {
    Current,
    Max,
    Total,
    Success,
    Failure,
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct InvestUser {
    pub id: i64,
    pub max: Credit,
    pub current: Credit,
    pub total: Credit,
    pub invest: (Credit, Credit), // success, failure
    pub active: bool,
}

impl InvestUser {
    pub fn new(id: i64) -> InvestUser {
        let mut user = InvestUser::default();
        user.id = id;
        user
    }
}

#[derive(Debug)]
pub struct InvestGame;

impl InvestGame {
    pub fn ensure_table(conn: &Connection) {
        conn.execute_batch(INVEST_TABLE)
            .expect("to create Invest table");
    }

    pub fn get_top_n(bound: i16, sort: &SortBy) -> Vec<InvestUser> {
        let conn = get_connection();

        macro_rules! un {
            ($e:expr, $n:expr) => {
                $e.get::<_, i64>($n) as usize
            };
        }

        let sort = match sort {
            SortBy::Current => "Current",
            SortBy::Max => "Max",
            SortBy::Total => "Total",
            SortBy::Success => "Success",
            SortBy::Failure => "Failure",
        };

        // TODO make this work for the other constraints
        let mut stmt = conn
            .prepare("SELECT * FROM Invest ORDER BY Current DESC LIMIT ?")
            .expect("valid sql");

        let mut iter = stmt
            .query_map(&[&bound], |row| InvestUser {
                id: row.get(0),
                max: un!(row, 1),
                current: un!(row, 2),
                total: un!(row, 3),
                invest: (un!(row, 4), un!(row, 5)),
                active: row.get(6),
            }).map_err(|e| { /* log this */ })
            .expect("to get rows");

        let mut out = vec![];
        while let Some(Ok(row)) = iter.next() {
            out.push(row)
        }
        out
    }

    pub fn find(id: i64) -> Option<InvestUser> {
        trace!("looking up id: {}", id);
        let conn = get_connection();
        Self::get_user_by_id(&conn, id).ok()
    }

    pub fn stats_for(id: i64) -> (InvestUser, usize) {
        let conn = get_connection();
        let user = Self::get_user_by_id(&conn, id).expect("to get user");
        let total = Self::get_collected(&conn);
        (user, total)
    }

    pub fn give(id: i64, credits: Credit) -> Option<Credit> {
        trace!("trying to give {}: {} credits", id, credits);
        let conn = get_connection();
        let mut user = Self::get_user_by_id(&conn, id).ok()?;
        user.current += credits; // TODO check for overflow...
        user.total += credits;

        let _ = Self::update_user(&conn, &user);
        Some(user.current)
    }

    pub fn take(id: i64, credits: Credit) -> Option<Credit> {
        trace!("trying to take {} credits from {}", credits, id);

        let conn = get_connection();
        let mut user = Self::get_user_by_id(&conn, id).ok()?;
        if credits > user.current {
            user.current = 0;
        } else {
            user.current -= credits;
        }

        let _ = Self::update_user(&conn, &user);
        Some(user.current)
    }

    pub fn set_active(id: i64) {
        const S: &str = r#"
            UPDATE Invest
            SET
                Active = 1
            WHERE ID = ?;
        "#;
        let conn = get_connection();
        let _ = conn.execute(S, &[&id]);
    }

    pub fn update(user: &InvestUser) {
        trace!("updating: {:?}", user);
        let conn = get_connection();
        let _ = Self::update_user(&conn, &user);
    }

    pub fn invest(chance: f64, id: i64, want: Credit) -> InvestResult<Investment> {
        trace!("id {} trying to invest {} at {}", id, want, chance);

        let conn = get_connection();
        let mut user = Self::get_user_by_id(&conn, id)
            .map_err(|_| InvestError::NotEnoughCredits { have: 0, want })?;

        if user.current < want {
            return Err(InvestError::NotEnoughCredits {
                have: user.current,
                want,
            });
        }

        if thread_rng().gen_bool(chance) {
            Self::failure(&conn, &mut user, want)
        } else {
            Self::success(&conn, &mut user, want)
        }
    }

    fn success(conn: &Connection, user: &mut InvestUser, want: Credit) -> InvestResult<Investment> {
        let old = user.current;
        user.current += want;
        user.total += want;
        user.invest.0 += 1;

        let _ = Self::update_user(&conn, &user);

        Ok(Investment::Success {
            old,
            new: user.current,
        })
    }

    fn failure(conn: &Connection, user: &mut InvestUser, want: Credit) -> InvestResult<Investment> {
        let old = user.current;
        user.current -= want;
        user.invest.1 += 1;

        Self::increment_collected(&conn, want);
        let _ = Self::update_user(&conn, &user);

        Ok(Investment::Failure {
            old,
            new: user.current,
        })
    }

    pub fn increment_all_active(conn: &Connection, amount: Credit) {
        const S: &str = r#"
            UPDATE Invest 
            SET 
                Current = Current + ?,
                Total = Total + ?
            WHERE Active = 1;
        "#;

        let _ = conn.execute(S, &[&(amount as i64), &(amount as i64)]);

        // TODO: probably should batch these
        Self::update_max(&conn);
    }

    fn update_max(conn: &Connection) {
        const S: &str = r#"
            UPDATE Invest
            SET
                Max = Current
            WHERE Max < Current;
        "#;

        let _ = conn.execute(S, &[]);
    }

    pub fn get_collected(conn: &Connection) -> Credit {
        let mut stmt = conn
            .prepare("SELECT Total FROM InvestStats WHERE ID = 0 LIMIT 1")
            .expect("valid sql");
        let mut iter = stmt
            .query_map(&[], |row| row.get::<_, i64>(0) as usize)
            .expect("to get total");
        iter.next().expect("to get total").expect("to get total")
    }

    pub fn increment_collected(conn: &Connection, amount: Credit) {
        const S: &str = "UPDATE InvestStats SET Total = Total + ? where ID = 0";
        conn.execute(S, &[&(amount as i64)])
            .expect("to update total");
    }

    pub fn get_user_by_id(conn: &Connection, id: i64) -> InvestResult<InvestUser> {
        let mut stmt = conn
            .prepare("SELECT * FROM Invest WHERE ID = ? LIMIT 1")
            .expect("valid sql");

        macro_rules! un {
            ($e:expr, $n:expr) => {
                $e.get::<_, i64>($n) as usize
            };
        }

        let mut iter = stmt
            .query_map(&[&id], |row| InvestUser {
                id: row.get(0),
                max: un!(row, 1),
                current: un!(row, 2),
                total: un!(row, 3),
                invest: (un!(row, 4), un!(row, 5)),
                active: row.get(6),
            }).map_err(|_err| InvestError::UserNotFound { id })?;

        if let Some(user) = iter.next() {
            return user.map_err(|_err| InvestError::UserNotFound { id });
        }

        let user = InvestUser::new(id);
        Self::create_user(&conn, &user)?;
        Ok(user)
    }

    pub fn update_user(conn: &Connection, user: &InvestUser) -> InvestResult<()> {
        use rusqlite::types::ToSql;

        const S: &str = r#"
            UPDATE Invest 
            SET 
                Max = ?,
                Current = ?,
                Total = ?,
                Success = ?,
                Failure = ?,
                Active = ?
            WHERE ID = ?"#;

        let map: &[&ToSql] = &[
            &(user.max as i64),
            &(user.current as i64),
            &(user.total as i64),
            &(user.invest.0 as i64),
            &(user.invest.1 as i64),
            &user.active,
            &user.id,
        ];

        let res = conn
            .execute(S, map)
            .map_err(|_err| InvestError::CannotInsert { id: user.id })
            .and_then(|_| Ok(()));

        if res.is_ok() {
            Self::update_max(&conn);
        }
        res
    }

    pub fn create_user(conn: &Connection, user: &InvestUser) -> InvestResult<()> {
        use rusqlite::types::ToSql;

        const S: &str = r#"
            INSERT OR IGNORE INTO Invest 
                (ID, Max, Current, Total, Success, Failure, Active) 
            VALUES (?, ?, ?, ?, ?, ?, ?)"#;

        let map: &[&ToSql] = &[
            &user.id,
            &(user.max as i64),
            &(user.current as i64),
            &(user.total as i64),
            &(user.invest.0 as i64),
            &(user.invest.1 as i64),
            &user.active,
        ];

        conn.execute(S, map)
            .map_err(|_err| InvestError::CannotInsert { id: user.id })
            .and_then(|_| Ok(()))
    }
}
