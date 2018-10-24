use crate::database::get_connection;

use rand::prelude::*;
use rusqlite::Connection; // TODO re-export this

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
            })
            .map_err(|e| { /* log this */ })
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
            })
            .map_err(|_err| InvestError::UserNotFound { id })?;

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
