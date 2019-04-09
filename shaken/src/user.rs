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
        let (user, bot) = match msg.command.as_str() {
            "PRIVMSG" | "WHISPER" => (
                User {
                    userid: msg.tags.get_userid()?,
                    display: msg.tags.get_display()?.to_string(),
                    color: msg.tags.get_color(),
                },
                false,
            ),
            "GLOBALUSERSTATE" => (
                User {
                    userid: msg.tags.get_userid()?,
                    display: msg.tags.get_display()?.to_string(),
                    color: RGB::from("fc0fc0"),
                },
                true,
            ),
            _ => return None,
        };

        let conn = database::get_connection();
        Some(UserStore::create_user(&conn, &user, bot))
    }
}

pub struct UserStore;
impl UserStore {
    fn ensure_table(conn: &Connection) {
        conn.execute_batch(USER_TABLE).expect("create Users table");
    }

    pub fn get_bot(conn: &Connection) -> Option<User> {
        Self::ensure_table(conn);
        let stmt = conn
            .prepare("SELECT ID, Display, Color FROM Users WHERE Self = ? COLLATE NOCASE LIMIT 1")
            .expect("valid sql");

        Self::get_user(&1, stmt)
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
            .query_map(&[q], |row| {
                Ok(User {
                    userid: row.get(0)?,
                    display: row.get(1)?,
                    color: RGB::from(&row.get::<_, String>(2)?),
                })
            })
            .map_err(|e| error!("cannot get user for '{}': {}", q, e))
            .ok()?;

        iter.next()?
            .map_err(|e| error!("cannot get user for '{}': {}", q, e))
            .ok()
    }

    pub fn update_color_for_id(conn: &Connection, id: i64, color: RGB) {
        Self::ensure_table(conn);

        match conn.execute(
            r#"UPDATE Users SET Color = ? where ID = ?"#,
            &[&color.to_string() as &dyn ToSql, &id],
        ) {
            Ok(_row) => {}
            Err(err) => error!("cannot insert {} into table: {}", id, err),
        };
    }

    pub fn create_user(conn: &Connection, user: &User, bot: bool) -> i64 {
        Self::ensure_table(conn);

        trace!("adding user: {:?} ({})", user, bot);
        let color = user.color.to_string();

        match conn.execute(
            r#"INSERT OR IGNORE INTO Users (ID, Display, Color, Self) VALUES (?, ?, ?, ?)"#,
            &[
                &user.userid as &dyn ToSql,
                &user.display,
                &color,
                &if bot { 1 } else { 0 },
            ],
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
    Color BLOB NOT NULL,                    -- their selected color (twitch, or custom. #RRGGBB)
    Self INTEGER NOT NULL                   -- this row is our bots identity
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
            false,
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
            false,
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
            false,
        );

        UserStore::update_color_for_id(&conn, 1005, crate::color::RGB::from("#FFFFFF"));
        let user = UserStore::get_user_by_id(&conn, 1005);
        assert_eq!(user, None);

        UserStore::update_color_for_id(&conn, 1004, crate::color::RGB::from("#FFFFFF"));
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
