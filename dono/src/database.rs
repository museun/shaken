use std::path::PathBuf;

use once_cell::sync::OnceCell;
pub static DB_PATH: OnceCell<PathBuf> = OnceCell::INIT;

// type Result<T> = std::result::Result<T, Error>;

// #[derive(Debug)]
// pub enum Error {
//     Rows(rusqlite::Error),
//     CurrentRow(rusqlite::Error),
//     PreviousRow(rusqlite::Error),
// }

pub fn get_connection() -> rusqlite::Connection {
    rusqlite::Connection::open(DB_PATH.get().unwrap()).expect("connect to database")
}

// let mut stmt = conn.prepare(include_str!("../sql/youtube/get_current.sql"))?;
// let mut stmt =
// conn.prepare(include_str!("../sql/youtube/get_previous.sql"))?; let mut stmt
// = conn.prepare(include_str!("../sql/youtube/get_all.sql"))?;
