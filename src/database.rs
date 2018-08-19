use crate::*;
use rusqlite::Connection;

#[cfg(not(test))]
pub fn get_connection() -> Connection {
    const DB_PATH: &str = "shaken.db";
    let conn = Connection::open(DB_PATH).unwrap();

    // TODO don't do this here
    UserStore::ensure_table(&conn);
    InvestGame::ensure_table(&conn);

    conn
}

#[cfg(test)]
pub fn get_connection() -> Connection {
    use rand::{distributions::Alphanumeric, prelude::*};
    use rusqlite::OpenFlags;

    thread_local!(static TEST_DB_ID: String = format!(
        "file:{}?mode=memory&cache=shared",
        thread_rng()
        .sample_iter(&Alphanumeric)
        .take(10)
        .collect::<String>()
    ));

    let conn = TEST_DB_ID.with(|id| {
        trace!("getting db conn: {}", id);

        Connection::open_with_flags(
            &id,
            OpenFlags::SQLITE_OPEN_URI | OpenFlags::SQLITE_OPEN_READ_WRITE,
        ).unwrap()
    });

    UserStore::ensure_table(&conn);
    InvestGame::ensure_table(&conn);
    conn
}
