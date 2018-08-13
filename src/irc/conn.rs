use std::cell::RefCell;
use std::collections::VecDeque;
use std::io::{self, prelude::*, BufRead, BufReader, BufWriter, Lines};
use std::net::{self, TcpStream, ToSocketAddrs};
use std::rc::Rc;
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

pub enum Conn {
    TcpConn(TcpConn),
    TestConn(Rc<TestConn>),
}

impl Conn {
    pub fn read(&self) -> Option<String> {
        match *self {
            Conn::TcpConn(ref conn) => conn.read(),
            Conn::TestConn(ref conn) => conn.read(),
        }
    }

    pub fn write(&self, data: &str) {
        match *self {
            Conn::TcpConn(ref conn) => conn.write(data),
            Conn::TestConn(ref conn) => conn.write(data),
        }
    }
}

impl From<TcpConn> for Conn {
    fn from(conn: TcpConn) -> Self {
        Conn::TcpConn(conn)
    }
}

impl From<Rc<TestConn>> for Conn {
    fn from(conn: Rc<TestConn>) -> Self {
        Conn::TestConn(Rc::clone(&conn))
    }
}

// REFACTOR: this could be parameterized for a Cursor to allow mocking
pub struct TcpConn {
    reader: RefCell<Lines<BufReader<TcpStream>>>,
    writer: RefCell<BufWriter<TcpStream>>,
}

impl TcpConn {
    pub fn new<A: ToSocketAddrs>(addr: A) -> Result<Self, ConnError> {
        let conn = TcpStream::connect(&addr).map_err(ConnError::CannotConnect)?;
        debug!("connected");

        let reader = {
            let conn = conn.try_clone().expect("to clone stream");
            RefCell::new(BufReader::new(conn).lines())
        };

        let writer = {
            let conn = conn.try_clone().expect("to clone stream");
            RefCell::new(BufWriter::new(conn))
        };

        Ok(Self { reader, writer })
    }

    pub fn write(&self, data: &str) {
        // XXX: might want to rate limit here
        let mut writer = self.writer.borrow_mut();
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

    pub fn read(&self) -> Option<String> {
        let mut reader = self.reader.borrow_mut();
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

#[derive(Debug, Clone, Default)]
pub struct TestConn {
    read: RefCell<VecDeque<String>>,
    write: RefCell<VecDeque<String>>,
}

impl TestConn {
    pub fn new() -> Rc<Self> {
        Rc::new(Self::default())
    }

    pub fn read(&self) -> Option<String> {
        let s = self.read.borrow_mut().pop_front();
        trace!("read: {:?}", s);
        s
    }

    pub fn write(&self, data: &str) {
        self.write.borrow_mut().push_back(data.into());
        trace!("write: {:?}", data);
    }

    // reads from the write queue (most recent)
    pub fn pop(&self) -> Option<String> {
        let s = self.write.borrow_mut().pop_back();
        debug!("pop: {:?}", s);
        s
    }

    // writes to the read queue
    pub fn push(&self, data: &str) {
        self.read.borrow_mut().push_back(data.into());
        debug!("push: {:?}", data);
    }

    pub fn read_len(&self) -> usize {
        self.read.borrow().len()
    }

    pub fn write_len(&self) -> usize {
        self.write.borrow().len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conn_read() {
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
    fn conn_write() {
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
