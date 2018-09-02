use std::collections::VecDeque;
use std::io::{self, prelude::*, BufRead, BufReader, BufWriter, Lines};
use std::net::{self, TcpStream, ToSocketAddrs};
use std::ops::{Deref, DerefMut};
use std::time::Duration;
use std::{fmt, str};

pub enum ConnError {
    InvalidAddress(net::AddrParseError),
    CannotConnect(io::Error),
}

impl fmt::Display for ConnError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ConnError::InvalidAddress(e) => write!(f, "invalid address: {}", e),
            ConnError::CannotConnect(e) => write!(f, "cannot connect: {}", e),
        }
    }
}

pub trait Conn: Send + Sync {
    fn try_read(&mut self) -> Option<Option<String>>;
    fn read(&mut self) -> Option<String>;
    fn write(&mut self, data: &str);
}

pub struct Connection<T> {
    conn: Box<T>,
}

impl<T> Connection<T> {
    pub fn new(c: T) -> Self {
        Connection { conn: Box::new(c) }
    }
}

impl<T> Deref for Connection<T> {
    type Target = Box<T>;

    fn deref(&self) -> &Self::Target {
        &self.conn
    }
}

impl<T> DerefMut for Connection<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.conn
    }
}

// REFACTOR: this could be parameterized for a Cursor to allow mocking
pub struct TcpConn {
    reader: Lines<BufReader<TcpStream>>,
    writer: BufWriter<TcpStream>,
}

impl TcpConn {
    pub fn new<A: ToSocketAddrs>(addr: A) -> Result<Self, ConnError> {
        let conn = TcpStream::connect(&addr).map_err(ConnError::CannotConnect)?;
        conn.set_read_timeout(Some(Duration::from_millis(50)))
            .expect("to set read timeout");

        debug!("connected");

        let reader = {
            let conn = conn.try_clone().expect("to clone stream");
            BufReader::new(conn).lines()
        };

        let writer = {
            let conn = conn.try_clone().expect("to clone stream");
            BufWriter::new(conn)
        };

        Ok(Self { reader, writer })
    }
}

impl Conn for TcpConn {
    fn write(&mut self, data: &str) {
        let writer = &mut self.writer;

        // XXX: might want to rate limit here
        for part in split(data) {
            // don't log the password
            if &part[..4] != "PASS" {
                let line = &part[..part.len() - 2];
                trace!("--> {}", &line); // trim the \r\n
            }

            trace!("trying to write to socket");
            if writer.write_all(part.as_bytes()).is_ok() {
                trace!("wrote to socket");
            } else {
                error!("cannot write to socket");
                return;
            }
        }
        writer.flush().expect("to flush");
    }

    // TODO: make a Result type for this
    fn try_read(&mut self) -> Option<Option<String>> {
        let reader = &mut self.reader;

        if let Some(line) = reader.next() {
            match line {
                Ok(line) => {
                    trace!("trying to read from socket");
                    trace!("<-- {}", &line);
                    Some(Some(line))
                }
                Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => Some(None),
                Err(ref e) if e.kind() == io::ErrorKind::TimedOut => Some(None),
                e => {
                    warn!("unknown error: {:?}", e);
                    None
                }
            }
        } else {
            warn!("couldn't read line");
            None
        }
    }

    fn read(&mut self) -> Option<String> {
        let reader = &mut self.reader;

        trace!("trying to read from socket");
        if let Some(Ok(line)) = reader.next() {
            trace!("<-- {}", &line);
            Some(line)
        } else {
            warn!("couldn't read line");
            None
        }
    }
}

fn split<S: AsRef<str>>(raw: S) -> Vec<String> {
    let raw = raw.as_ref();

    if raw.len() > 510 - 2 && raw.contains(':') {
        let split = raw.splitn(2, ':').map(|s| s.trim()).collect::<Vec<_>>();
        let (head, tail) = (split[0], split[1]);
        let mut vec = vec![];
        for part in tail
            .as_bytes()
            .chunks(510 - head.len() - 2)
            .map(str::from_utf8)
        {
            match part {
                Ok(part) => vec.push(format!("{} :{}\r\n", head, part)),
                Err(err) => {
                    warn!("dropping a slice: {}", err);
                    continue;
                }
            }
        }
        vec
    } else {
        vec![format!("{}\r\n", raw)]
    }
}

#[derive(Debug, Default)]
pub struct TestConn {
    read: VecDeque<String>,
    write: VecDeque<String>,
}

impl TestConn {
    pub fn new() -> Self {
        Self::default()
    }

    // reads from the write queue (most recent)
    pub fn pop(&mut self) -> Option<String> {
        let s = self.write.pop_front();
        debug!("pop: {:?}", s);
        s
    }

    // writes to the read queue
    pub fn push(&mut self, data: &str) {
        self.read.push_back(data.into());
        debug!("push: {:?}", data);
    }

    pub fn read_len(&self) -> usize {
        self.read.len()
    }

    pub fn write_len(&self) -> usize {
        self.write.len()
    }
}

impl Conn for TestConn {
    fn try_read(&mut self) -> Option<Option<String>> {
        self.read().and_then(|s| Some(Some(s)))
    }

    fn read(&mut self) -> Option<String> {
        let s = self.read.pop_front();
        trace!("read: {:?}", s);
        s
    }

    fn write(&mut self, data: &str) {
        self.write.push_back(data.into());
        trace!("write: {:?}", data);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conn_read() {
        let mut conn = TestConn::new();
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
    fn conn_write() {
        let mut conn = TestConn::new();
        assert!(conn.pop().is_none());

        let list = &["a", "b", "c", "d"];
        for s in list {
            conn.write(s);
        }

        assert_eq!(conn.read_len(), 0);
        assert_eq!(conn.write_len(), 4);

        for s in list.iter() {
            assert_eq!(conn.pop(), Some(s.to_string()));
        }
    }
}
