use std::path::PathBuf;

use once_cell::sync::OnceCell;
pub static DB_PATH: OnceCell<PathBuf> = OnceCell::INIT;

pub fn get_connection() -> rusqlite::Connection {
    if cfg!(test) {
        // do something here
        panic!("no dbs in test")
    }

    rusqlite::Connection::open(DB_PATH.get().unwrap()).expect("connect to database")
}
