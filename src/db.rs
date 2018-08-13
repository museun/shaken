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
    use rusqlite;

    let conn = Connection::open_with_flags(
        "file::memory:?cache=shared",
        rusqlite::OpenFlags::SQLITE_OPEN_URI | rusqlite::OpenFlags::SQLITE_OPEN_READ_WRITE,
    ).unwrap();
    UserStore::init_table(&conn);
    conn
}
