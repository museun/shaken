// #![allow(dead_code)]
use crate::prelude::*;

use std::time::Instant;

use crossbeam_channel as channel;
use rusqlite::Connection;

pub fn init_logger() {
    let _ = env_logger::Builder::from_default_env()
        .default_format_timestamp(false)
        .try_init();
}

/// don't use 42 (bot) or 1000 (you)
pub fn make_test_user(conn: &Connection, name: &str, id: i64) -> User {
    let user = User {
        display: name.into(),
        userid: id,
        color: crate::color::RGB::from("#ffffff"),
    };
    let _ = UserStore::create_user(&conn, &user);
    user
}

const USER_ID: i64 = 1000;
const USER_NAME: &str = "test";

pub struct Environment<'a> {
    bot: Bot<'a, TestConn>,
    db: Connection,
    tx: channel::Sender<Instant>,
    rx: channel::Receiver<Instant>,
}

impl<'a> Default for Environment<'a> {
    fn default() -> Self {
        Self::new()
    }
}

impl<'a> Environment<'a> {
    pub fn new() -> Self {
        let conn = TestConn::new();
        use crate::{
            color::RGB,
            user::{User, UserStore},
        };

        // db gets dropped
        let db = crate::database::get_connection();
        UserStore::create_user(
            &db,
            &User {
                display: USER_NAME.into(),
                color: RGB::from("#f0f0f0"),
                userid: USER_ID,
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

        let (tx, rx) = channel::bounded(16);

        Self {
            bot: Bot::new(conn),
            db,
            tx,
            rx,
        }
    }

    pub fn get_db_conn(&self) -> &Connection {
        &self.db
    }

    pub fn add(&mut self, m: &'a dyn Module) {
        self.bot.add(m)
    }

    pub fn step(&mut self) {
        let msg = {
            let conn = self.bot.get_conn_mut();
            let conn = &mut *conn.lock();
            ReadType::Message(conn.read().unwrap())
        };

        let _ = self.bot.step(&msg);
    }

    pub fn tick(&self) {
        let msg = ReadType::Tick(Instant::now());
        let _ = self.bot.step(&msg);
    }

    pub fn push(&mut self, data: &str) {
        let conn = self.bot.get_conn_mut();
        let conn = &mut *conn.lock();
        conn.push(&format!(
            "user-id={};display-name={};color=#FFFFFF :{}!user@irc.test PRIVMSG #test :{}",
            USER_ID, USER_NAME, USER_NAME, data
        ))
    }

    pub fn push_user(&mut self, data: &str, user: (&str, i64)) {
        UserStore::create_user(
            &self.db,
            &User {
                display: user.0.into(),
                color: crate::color::RGB::from("#f0f0f0"),
                userid: user.1,
            },
        );

        let conn = self.bot.get_conn_mut();
        let conn = &mut *conn.lock();
        conn.push(&format!(
            "user-id={};display-name={};color=#FFFFFF :{}!user@irc.test PRIVMSG #test :{}",
            user.1, user.0, user.0, data
        ))
    }

    pub fn push_owner(&mut self, data: &str) {
        let conn = self.bot.get_conn_mut();
        let conn = &mut *conn.lock();
        conn.push(&format!(
            "user-id={};display-name={};color=#FFFFFF :{}!user@irc.test PRIVMSG #test :{}",
            23196011, USER_NAME, USER_NAME, data
        ))
    }

    pub fn push_raw(&mut self, data: &str) {
        let conn = self.bot.get_conn_mut();
        let conn = &mut *conn.lock();
        conn.push(data)
    }

    pub fn pop_raw(&mut self) -> Option<String> {
        let conn = self.bot.get_conn_mut();
        let conn = &mut *conn.lock();
        conn.pop()
    }

    pub fn pop(&mut self) -> Option<String> {
        let conn = self.bot.get_conn_mut();
        let conn = &mut *conn.lock();
        let mut data = conn.pop()?;
        data.insert_str(0, ":test!user@irc.test ");
        let msg = Message::parse(&data);
        Some(msg.data)
    }

    pub fn get_user_id(&self) -> i64 {
        USER_ID
    }

    pub fn get_user_name(&self) -> &str {
        USER_NAME
    }

    pub fn drain(&mut self) {
        while let Some(_) = self.pop() {}
    }

    pub fn drain_and_log(&mut self) {
        while let Some(resp) = self.pop() {
            warn!("{:#?}", resp);
        }
    }
}
