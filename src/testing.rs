use crate::irc::{Message, TestConn};
use crate::*;

use crossbeam_channel as channel;
use rusqlite::Connection;
use std::rc::Rc;
use std::time::Instant;

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
    conn: Rc<TestConn>,
    bot: Bot<'a>,
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
        use crate::{color::RGB, user::User, user::UserStore};

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
            conn: Rc::clone(&conn),
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

    pub fn step(&self) {
        let _ = self.bot.step(&self.rx.clone());
    }

    pub fn tick(&self) {
        self.tx.send(Instant::now());
        let _ = self.bot.step(&self.rx.clone());
    }

    pub fn push(&self, data: &str) {
        self.conn.push(&format!(
            "user-id={};display-name={};color=#FFFFFF :{}!user@irc.test PRIVMSG #test :{}",
            USER_ID, USER_NAME, USER_NAME, data
        ))
    }

    pub fn push_user(&self, data: &str, user: (&str, i64)) {
        UserStore::create_user(
            &self.db,
            &User {
                display: user.0.into(),
                color: crate::color::RGB::from("#f0f0f0"),
                userid: user.1,
            },
        );

        self.conn.push(&format!(
            "user-id={};display-name={};color=#FFFFFF :{}!user@irc.test PRIVMSG #test :{}",
            user.1, user.0, user.0, data
        ))
    }

    pub fn push_owner(&self, data: &str) {
        self.conn.push(&format!(
            "user-id={};display-name={};color=#FFFFFF :{}!user@irc.test PRIVMSG #test :{}",
            23196011, USER_NAME, USER_NAME, data
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
        // TODO make this use USER_NAME
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

    pub fn drain(&self) {
        while let Some(_) = self.pop() {}
    }

    pub fn drain_and_log(&self) {
        while let Some(resp) = self.pop() {
            warn!("{:#?}", resp);
        }
    }
}
