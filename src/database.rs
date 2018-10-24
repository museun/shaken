use rusqlite::Connection;

pub fn ensure_table(f: fn(&Connection)) -> Connection {
    let conn = get_connection();
    f(&conn);
    conn
}

#[cfg(not(test))]
pub fn get_connection() -> Connection {
    const DB_PATH: &str = "shaken.db";
    Connection::open(DB_PATH).unwrap()
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

    TEST_DB_ID.with(|id| {
        trace!("getting db conn: {}", id);

        Connection::open_with_flags(
            &id,
            OpenFlags::SQLITE_OPEN_URI | OpenFlags::SQLITE_OPEN_READ_WRITE,
        )
        .unwrap()
    })
}
