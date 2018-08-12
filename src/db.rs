use crate::UserStore;
use rusqlite::Connection;

const DB_PATH: &str = "shaken.db";

#[cfg(not(test))]
pub fn get_connection() -> Connection {
    let conn = Connection::open(DB_PATH).unwrap();
    UserStore::init_table(&conn);
    conn
}

#[cfg(test)]
pub fn get_connection() -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    UserStore::init_table(&conn);
    conn
}
