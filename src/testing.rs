#![allow(dead_code)]
use std::collections::VecDeque;
use std::sync::Arc;

use parking_lot::RwLock;

use crate::{bot, config, message};

pub fn init_logger() {
    let _ = env_logger::Builder::from_default_env()
        .default_format_timestamp(false)
        .try_init();
}

pub struct Environment {
    pub conn: Arc<TestConn>,
    pub bot: bot::Bot,
    pub config: config::Config,
    pub user_id: String,
}

impl Environment {
    pub fn new() -> Self {
        use crate::{bot::Bot, config::Config, conn::Conn};
        let conn = Arc::new(TestConn::new());
        Self {
            conn: Arc::clone(&conn),
            bot: Bot::new(Conn::TestConn(Arc::clone(&conn)), &Config::default()),
            config: Config::default(),
            user_id: "1004".into(),
        }
    }

    // although this is going thru a mutex, might as well do a mutable borrow to keep the surprises low
    pub fn set_owner(&mut self, id: &str) {
        let mut inner = self.bot.inner.lock();
        inner.owners.push(id.to_string())
    }

    pub fn set_user_id(&mut self, id: &str) {
        self.user_id = id.to_string();
    }

    pub fn step(&self) {
        let msg = {
            match self.bot.conn.read() {
                Some(line) => message::Message::parse(&line),
                None => return,
            }
        };

        self.bot.dispatch(&msg, false);
    }

    pub fn tick(&self) {
        self.bot.dispatch(&message::Message::default(), true);
    }

    pub fn push_privmsg(&self, data: &str) {
        self.conn.push(&format!(
            "user-id={} :test!user@irc.test PRIVMSG #test :{}",
            self.user_id, data
        ))
    }

    pub fn push_user_context(&self, user: &bot::User, data: &str) {
        self.conn.push(&format!(
            "user-id={};color={};display-name={} :{}!user@irc.test PRIVMSG #test :{}",
            &user.userid, &user.color, &user.display, &user.display, data
        ))
    }

    pub fn pop_msg(&self) -> Option<message::Message> {
        let mut data = self.conn.pop()?.to_string();
        data.insert_str(0, ":test!user@irc.test ");
        Some(message::Message::parse(&data))
    }

    pub fn pop_env(&self) -> Option<message::Envelope> {
        let mut data = self.conn.pop()?.to_string();
        data.insert_str(0, ":test!user@irc.test ");
        Some(message::Envelope::from_msg(&message::Message::parse(&data)))
    }

    pub fn drain_envs(&self) {
        while let Some(_) = self.pop_env() {}
    }

    pub fn drain_env_warn_log(&self) {
        while let Some(env) = self.pop_env() {
            warn!("{:#?}", env);
        }
    }

    pub fn drain_msgs(&self) {
        while let Some(_) = self.pop_msg() {}
    }
}

#[derive(Debug, Default)]
pub struct TestConn {
    read: RwLock<VecDeque<String>>,
    write: RwLock<VecDeque<String>>,
}

impl TestConn {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn read(&self) -> Option<String> {
        let s = self.read.write().pop_front();
        trace!("read: {:?}", s);
        s
    }

    pub fn write(&self, data: &str) {
        self.write.write().push_back(data.into());
        trace!("write: {:?}", data);
    }

    pub fn next_line(&self) {}

    // reads from the write queue (most recent)
    pub fn pop(&self) -> Option<String> {
        let s = self.write.write().pop_back();
        debug!("pop: {:?}", s);
        s
    }

    // writes to the read queue
    pub fn push(&self, data: &str) {
        self.read.write().push_back(data.into());
        debug!("push: {:?}", data);
    }

    pub fn read_len(&self) -> usize {
        self.read.read().len()
    }

    pub fn write_len(&self) -> usize {
        self.write.read().len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_conn_read() {
        let conn = TestConn::new();
        assert!(conn.read().is_none());

        let list = &["a", "b", "c", "d"];
        for s in list {
            conn.push(s);
        }

        assert_eq!(conn.read_len(), 4);
        assert_eq!(conn.write_len(), 0);

        for s in list {
            assert_eq!(conn.read(), Some(s.to_string()));
        }
    }

    #[test]
    fn test_conn_write() {
        let conn = TestConn::new();
        assert!(conn.pop().is_none());

        let list = &["a", "b", "c", "d"];
        for s in list {
            conn.write(s);
        }

        assert_eq!(conn.read_len(), 0);
        assert_eq!(conn.write_len(), 4);

        for s in list.iter().rev() {
            assert_eq!(conn.pop(), Some(s.to_string()));
        }
    }
}
