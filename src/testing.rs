use crate::irc::{Message, TestConn};
use crate::*;

use rusqlite::Connection;
use std::rc::Rc;

pub fn init_logger() {
    let _ = env_logger::Builder::from_default_env()
        .default_format_timestamp(false)
        .try_init();
}

pub struct Environment<'a> {
    conn: Rc<TestConn>,
    bot: Bot<'a>,
    db: Connection,
}

impl<'a> Environment<'a> {
    pub fn new() -> Self {
        let conn = TestConn::new();

        use crate::{color::RGB, user::User, user::UserStore};
        // db gets dropped
        let db = crate::database::get_connection();
        UserStore::create_user(
            &db,
            &User {
                display: "test".into(),
                color: RGB::from("#f0f0f0"),
                userid: 1000,
            },
        );
        UserStore::create_user(
            &db,
            &User {
                display: "shaken_bot".into(),
                color: RGB::from("#f0f0f0"),
                userid: 42,
            },
        );

        Self {
            conn: Rc::clone(&conn),
            bot: Bot::new(conn),
            db,
        }
    }

    pub fn get_db_conn(&self) -> &Connection {
        &self.db
    }

    pub fn add(&mut self, m: &'a Box<dyn Module>) {
        self.bot.add(m)
    }

    pub fn step(&self) {
        self.bot.step()
    }

    pub fn push(&self, data: &str) {
        self.conn.push(&format!(
            "user-id={};display-name=test;color=#FFFFFF :test!user@irc.test PRIVMSG #test :{}",
            1000, data
        ))
    }

    pub fn push_raw(&self, data: &str) {
        self.conn.push(data)
    }

    pub fn pop_raw(&self) -> Option<String> {
        self.conn.pop()
    }

    pub fn pop(&self) -> Option<String> {
        let mut data = self.conn.pop()?;
        data.insert_str(0, ":test!user@irc.test ");
        let msg = Message::parse(&data);
        Some(msg.data)
    }

    pub fn drain(&self) {
        while let Some(_) = self.pop() {}
    }

    pub fn drain_and_log(&self) {
        while let Some(resp) = self.pop() {
            warn!("{:#?}", resp);
        }
    }
}
