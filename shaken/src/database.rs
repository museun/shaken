use rusqlite::Connection;

pub fn ensure_table(f: fn(&Connection)) -> Connection {
    let conn = get_connection();
    f(&conn);
    conn
}

#[cfg(not(test))]
pub fn get_connection() -> Connection {
    use directories::ProjectDirs;
    let dir = ProjectDirs::from("com.github", "museun", "shaken")
        .and_then(|dir| {
            let dir = dir.data_dir();
            std::fs::create_dir_all(&dir)
                .ok()
                .and_then(|_| Some(dir.join("shaken.db")))
        })
        .expect("data dir should be available to store bot data files");
    Connection::open(dir).unwrap()
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
        Connection::open_with_flags(
            &id,
            OpenFlags::SQLITE_OPEN_URI | OpenFlags::SQLITE_OPEN_READ_WRITE,
        )
        .unwrap()
    })
}
