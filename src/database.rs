use crate::*;
use rusqlite::Connection;

#[cfg(not(test))]
pub fn get_connection() -> Connection {
    const DB_PATH: &str = "shaken.db";
    let conn = Connection::open(DB_PATH).unwrap();
    UserStore::init_table(&conn);
    conn
}

#[cfg(test)]
pub fn get_connection() -> Connection {
    use rusqlite::OpenFlags;

    let conn = Connection::open_with_flags(
        "file::memory:?cache=shared",
        OpenFlags::SQLITE_OPEN_URI | OpenFlags::SQLITE_OPEN_READ_WRITE,
    ).unwrap();
    UserStore::init_table(&conn);
    conn
}
