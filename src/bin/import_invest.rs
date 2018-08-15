extern crate rusqlite;
use rusqlite::Connection;

extern crate serde;
#[macro_use]
extern crate serde_derive;
extern crate serde_json;

use std::collections::HashMap;
use std::fs;

fn main() {
    const DB_PATH: &str = "shaken.db";
    let conn = Connection::open(DB_PATH).unwrap();

    conn.execute_batch(INVEST_TABLE)
        .expect("to create Invests table");

    let previous = fs::read_to_string("invest.json").expect("to read invest.json");

    #[derive(Deserialize, Serialize, Debug)]
    struct InvestUser {
        id: String,
        max: i64,
        current: i64,
        total: i64,
        invest: (i64, i64), // success, failure
    }
    #[derive(Deserialize, Serialize, Debug)]
    struct Invest {
        total: usize, // total lost
        state: HashMap<String, InvestUser>,
    }

    let data: Invest = serde_json::from_str(&previous).expect("to parse json");

    fn add_user(conn: &Connection, user: &InvestUser) {
        let id = user.id.parse::<i64>().expect("to parse i64");

        match conn.execute(
            r#"INSERT OR IGNORE INTO Invest (ID, Max, Current, Total, Success, Failure, Active) VALUES (?, ?, ?, ?, ?, ?, ?)"#,
            &[&id, &user.max, &user.current, &user.total, &user.invest.0, &user.invest.1, &0],
        ) {
            Ok(row) if row == 0 => eprintln!("user({:?}) already exists", user),
            Ok(row) => eprintln!("added user({:?}) at {}", user, row),
            Err(err) => eprintln!("cannot insert user({:?}) into table: {}", user, err),
        };
    }

    for (_name, user) in data.state {
        add_user(&conn, &user)
    }

    fn increment_total_invested(conn: &Connection, amount: i64) {
        const S: &str = "UPDATE InvestStats SET Total = ? where ID = 0";
        conn.execute(S, &[&amount]).expect("to update total");
    }

    increment_total_invested(&conn, data.total as i64);
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
