use crate::prelude::*;
use log::*;
use rusqlite::{self, types::ToSql, Connection};

#[derive(Clone, PartialEq, Debug)]
pub struct User {
    pub userid: i64,
    pub display: String,
    pub color: RGB,
}

impl User {
    pub fn from_msg(msg: &irc::Message) -> Option<i64> {
        let user = match msg.command.as_str() {
            "PRIVMSG" | "WHISPER" => User {
                userid: msg.tags.get_userid()?,
                display: msg.tags.get_display()?.to_string(),
                color: msg.tags.get_color()?,
            },
            "GLOBALUSERSTATE" => User {
                userid: msg.tags.get_userid()?,
                display: msg.tags.get_display()?.to_string(),
                // TODO get this from the config
                // or from the database
                color: RGB::from("fc0fc0"),
            },
            _ => return None,
        };

        let conn = database::get_connection();
        Some(UserStore::create_user(&conn, &user))
    }
}

pub struct UserStore;
impl UserStore {
    fn ensure_table(conn: &Connection) {
        conn.execute_batch(USER_TABLE)
            .expect("to create Users table");
    }

    // XXX: default color for the bot: fc0fc0
    pub fn get_bot(conn: &Connection, name: &str) -> Option<User> {
        Self::ensure_table(conn);
        let stmt = conn
            .prepare(
                "SELECT ID, Display, Color FROM Users WHERE DISPLAY = ? COLLATE NOCASE LIMIT 1",
            )
            .expect("valid sql");

        Self::get_user(&name, stmt)
    }

    pub fn get_user_by_id(conn: &Connection, id: i64) -> Option<User> {
        Self::ensure_table(conn);
        let stmt = conn
            .prepare("SELECT ID, Display, Color FROM Users WHERE ID = ? LIMIT 1")
            .expect("valid sql");

        Self::get_user(&id, stmt)
    }

    pub fn get_user_by_name(conn: &Connection, name: &str) -> Option<User> {
        Self::ensure_table(conn);
        let stmt = conn
            .prepare(
                "SELECT ID, Display, Color FROM Users WHERE DISPLAY = ? COLLATE NOCASE LIMIT 1",
            )
            .expect("valid sql");

        Self::get_user(&name, stmt)
    }

    fn get_user<T>(q: &T, mut stmt: rusqlite::Statement<'_>) -> Option<User>
    where
        T: ::std::fmt::Display + rusqlite::types::ToSql,
    {
        let mut iter = stmt
            .query_map(&[q], |row| User {
                userid: row.get(0),
                display: row.get(1),
                color: RGB::from(&row.get::<_, String>(2)),
            })
            .map_err(|e| error!("cannot get user for '{}': {}", q, e))
            .ok()?;

        iter.next()?
            .map_err(|e| error!("cannot get user for '{}': {}", q, e))
            .ok()
    }

    pub fn update_color_for_id(conn: &Connection, id: i64, color: &RGB) {
        Self::ensure_table(conn);

        match conn.execute(
            r#"UPDATE Users SET Color = ? where ID = ?"#,
            &[&color.to_string() as &dyn ToSql, &id],
        ) {
            Ok(_row) => {}
            Err(err) => error!("cannot insert {} into table: {}", id, err),
        };
    }

    pub fn create_user(conn: &Connection, user: &User) -> i64 {
        Self::ensure_table(conn);

        let color = user.color.to_string();

        match conn.execute(
            r#"INSERT OR IGNORE INTO Users (ID, Display, Color) VALUES (?, ?, ?)"#,
            &[&user.userid as &dyn ToSql, &user.display, &color],
        ) {
            Ok(row) if row == 0 => {}
            Ok(_row) => {}
            Err(err) => error!("cannot insert user({:?}) into table: {}", user, err),
        };

        match conn.execute(
            r#"UPDATE Users SET Display = ? where ID = ?"#,
            &[&user.display as &dyn ToSql, &user.userid],
        ) {
            Ok(_row) => {}
            Err(err) => error!("cannot insert user({:?}) into table: {}", user, err),
        };

        user.userid
    }
}

const USER_TABLE: &str = r#"
CREATE TABLE IF NOT EXISTS Users (
    ID INTEGER PRIMARY KEY NOT NULL UNIQUE, -- twitch ID
    Display TEXT NOT NULL,                  -- twitch display name
    Color BLOB NOT NULL                     -- their selected color (twitch, or custom. #RRGGBB)
);
"#;

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    #[test]
    fn userstore_stuff() {
        let conn = Connection::open_in_memory().unwrap();
        UserStore::ensure_table(&conn);

        let user = UserStore::get_user_by_id(&conn, 1004);
        assert_eq!(user, None);

        UserStore::create_user(
            &conn,
            &User {
                display: "Test".into(),
                color: RGB::from("#f0f0f0"),
                userid: 1004,
            },
        );

        let user = UserStore::get_user_by_id(&conn, 1004);
        assert_eq!(
            user,
            Some(User {
                display: "Test".into(),
                color: RGB::from("#f0f0f0"),
                userid: 1004,
            })
        );

        let user = UserStore::get_user_by_name(&conn, "test");
        assert_eq!(
            user,
            Some(User {
                display: "Test".into(),
                color: RGB::from("#f0f0f0"),
                userid: 1004,
            })
        );

        let user = UserStore::get_user_by_name(&conn, "TEST");
        assert_eq!(
            user,
            Some(User {
                display: "Test".into(),
                color: RGB::from("#f0f0f0"),
                userid: 1004,
            })
        );

        let user = UserStore::get_user_by_name(&conn, "not_test");
        assert_eq!(user, None);

        UserStore::create_user(
            &conn,
            &User {
                display: "TEST".into(),
                color: RGB::from("#abcabc"),
                userid: 1004,
            },
        );

        let user = UserStore::get_user_by_name(&conn, "test");
        assert_eq!(
            user,
            Some(User {
                display: "TEST".into(),
                color: RGB::from("#f0f0f0"),
                userid: 1004,
            })
        );

        UserStore::create_user(
            &conn,
            &User {
                display: "TEST".into(),
                color: RGB::from("#abcabc"),
                userid: 1004,
            },
        );

        UserStore::update_color_for_id(&conn, 1005, &crate::color::RGB::from("#FFFFFF"));
        let user = UserStore::get_user_by_id(&conn, 1005);
        assert_eq!(user, None);

        UserStore::update_color_for_id(&conn, 1004, &crate::color::RGB::from("#FFFFFF"));
        let user = UserStore::get_user_by_id(&conn, 1004);
        assert_eq!(
            user,
            Some(User {
                display: "TEST".into(),
                color: RGB::from("#FFFFFF"),
                userid: 1004,
            })
        );
    }
}
