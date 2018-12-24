#![allow(dead_code)]
use crate::prelude::*;

use std::collections::VecDeque;

use crossbeam_channel as channel;
use crossbeam_channel::select;
use log::*;
use rusqlite::Connection;
use simplelog::{Config as LogConfig, LevelFilter, TermLogger};

pub enum LogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

pub fn init_test_logger(filter: Option<LogLevel>) {
    let filter = match filter.unwrap_or(LogLevel::Trace) {
        LogLevel::Trace => LevelFilter::Trace,
        LogLevel::Debug => LevelFilter::Debug,
        LogLevel::Info => LevelFilter::Info,
        LogLevel::Warn => LevelFilter::Warn,
        LogLevel::Error => LevelFilter::Error,
    };

    TermLogger::init(filter, LogConfig::default()).expect("initialize logger");
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
    db: &'a Connection,
    module: &'a mut dyn Module,

    read: VecDeque<String>,
    write: VecDeque<String>,

    in_tx: channel::Sender<(Option<irc::Message>, Response)>,
    in_rx: channel::Receiver<(Option<irc::Message>, Response)>,
}

impl<'a> Environment<'a> {
    pub fn new<M: Module + 'a>(db: &'a Connection, module: &'a mut M) -> Self {
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

        let (in_tx, in_rx) = channel::unbounded();
        Self {
            db,
            module,
            read: VecDeque::new(),
            write: VecDeque::new(),
            in_tx,
            in_rx,
        }
    }

    pub fn get_db_conn(&self) -> &Connection {
        &self.db
    }

    pub fn module(&self) -> &dyn Module {
        &*self.module
    }

    pub fn module_mut(&mut self) -> &mut dyn Module {
        self.module
    }

    pub fn step_wait(&mut self, wait: bool) {
        let input = match self.read.pop_front() {
            Some(data) => data,
            _ => panic!("must be able to read for step"),
        };

        let (out_tx, out_rx) = channel::unbounded();

        let msg = irc::Message::parse(&input);
        let req = Request::try_from(&msg);
        trace!("(msg) -> {:?}", msg);
        trace!("(req) -> {:?}", req);

        let _ = out_tx.send(Event::Message(msg, req.map(Box::new)));

        drop(out_tx);
        self.module.handle(out_rx, self.in_tx.clone());

        let msg = if wait {
            use std::time::Duration;
            select! {
                recv(self.in_rx) -> msg => msg.ok(),
                recv(channel::after(Duration::from_millis(5000))) -> _ => panic!("test timed out")
            }
        } else {
            self.in_rx.try_recv().ok()
        };

        if let Some((msg, resp)) = msg {
            if let Some(msg) = msg.as_ref() {
                self.module.inspect(&msg.clone(), &resp.clone());
            }
            if let Some(resp) = resp.build(msg.as_ref()) {
                resp.into_iter()
                    .inspect(|s| trace!("writing response: {}", s))
                    .for_each(|m| self.write.push_back(m));
            }
        }
    }

    pub fn step(&mut self) {
        self.step_wait(true)
    }

    pub fn tick(&mut self) {
        use std::time::{Duration, Instant};

        let (out_tx, out_rx) = channel::unbounded();
        let _ = out_tx.send(Event::Tick(Instant::now()));

        drop(out_tx);
        self.module.handle(out_rx, self.in_tx.clone());

        let msg = select! {
            recv(self.in_rx) -> msg => msg,
            recv(channel::after(Duration::from_millis(5000))) -> _ => panic!("test timed out")
        };

        if let Ok((msg, resp)) = msg {
            if let Some(msg) = msg.as_ref() {
                self.module.inspect(&msg.clone(), &resp.clone());
            }
            if let Some(resp) = resp.build(msg.as_ref()) {
                resp.into_iter()
                    .inspect(|s| trace!("writing response: {}", s))
                    .for_each(|m| self.write.push_back(m));
            }
        }
    }

    pub fn push(&mut self, data: &str) {
        self.read.push_back(format!(
            "@user-id={};display-name={};color=#FFFFFF :{}!user@irc.test PRIVMSG #test :{}",
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

        self.push_raw(&format!(
            "@user-id={};display-name={};color=#FFFFFF :{}!user@irc.test PRIVMSG #test :{}",
            user.1, user.0, user.0, data
        ))
    }

    pub fn push_mod(&mut self, data: &str) {
        self.push_raw(&format!(
            "@badges=moderator/1;user-id={};display-name={};color=#FFFFFF :{}!user@irc.test \
             PRIVMSG #test :{}",
            USER_ID, USER_NAME, USER_NAME, data
        ))
    }

    pub fn push_broadcaster(&mut self, data: &str) {
        self.push_raw(&format!(
            "@badges=broadcaster/1;user-id={};display-name={};color=#FFFFFF :{}!user@irc.test \
             PRIVMSG #test :{}",
            USER_ID, USER_NAME, USER_NAME, data
        ))
    }
    pub fn push_owner(&mut self, data: &str) {
        self.push_raw(&format!(
            "@badges=turbo/1;user-id={};display-name={};color=#FFFFFF :{}!user@irc.test PRIVMSG \
             #test :{}",
            23196011, USER_NAME, USER_NAME, data
        ))
    }

    pub fn push_raw(&mut self, data: &str) {
        trace!("<- {}", data);
        self.read.push_back(data.to_string())
    }

    pub fn pop_raw(&mut self) -> Option<String> {
        self.write.pop_front()
    }

    pub fn pop(&mut self) -> Option<String> {
        let mut data = self.write.pop_front()?;
        data.insert_str(0, ":test!user@irc.test ");
        let msg = irc::Message::parse(&data);
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

    /// this logs to the warn level
    pub fn drain_and_log(&mut self) {
        while let Some(resp) = self.pop() {
            warn!("{:#?}", resp);
        }
    }
}
