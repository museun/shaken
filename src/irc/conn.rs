use log::*;
use std::io::{self, prelude::*, BufRead, BufReader, BufWriter, Lines};
use std::net::{self, TcpStream, ToSocketAddrs};
use std::time::Duration;
use std::{fmt, str};

pub enum ConnError {
    InvalidAddress(net::AddrParseError),
    CannotConnect(io::Error),
}

impl fmt::Display for ConnError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConnError::InvalidAddress(e) => write!(f, "invalid address: {}", e),
            ConnError::CannotConnect(e) => write!(f, "cannot connect: {}", e),
        }
    }
}

pub enum ReadStatus {
    Data(String),
    Nothing,
}

pub struct TcpConn {
    reader: Lines<BufReader<TcpStream>>,
    writer: BufWriter<TcpStream>,
}

impl TcpConn {
    pub fn connect<A: ToSocketAddrs>(addr: A) -> Result<Self, ConnError> {
        let conn = TcpStream::connect(&addr).map_err(ConnError::CannotConnect)?;
        conn.set_read_timeout(Some(Duration::from_millis(50)))
            .expect("set read timeout");

        debug!("connected");

        let reader = {
            let conn = conn.try_clone().expect("clone stream");
            BufReader::new(conn).lines()
        };

        let writer = {
            let conn = conn.try_clone().expect("clone stream");
            BufWriter::new(conn)
        };

        Ok(Self { reader, writer })
    }

    pub fn write(&mut self, data: &str) {
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
        writer.flush().expect("flush");
    }

    pub fn try_read(&mut self) -> Option<ReadStatus> {
        let reader = &mut self.reader;

        if let Some(line) = reader.next() {
            return match line {
                Ok(line) => {
                    trace!("trying to read from socket");
                    trace!("<-- {}", &line);
                    Some(ReadStatus::Data(line))
                }
                Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => Some(ReadStatus::Nothing),
                // TODO read docs on iocp to make sure this isn't a real error
                Err(ref e) if e.kind() == io::ErrorKind::TimedOut => Some(ReadStatus::Nothing),
                Err(e) => {
                    warn!("unknown error: {:?}", e);
                    None
                }
            };
        };

        warn!("couldn't read line");
        None
    }
}

#[inline]
fn split(raw: &str) -> Vec<String> {
    if raw.len() > 510 - 2 && raw.contains(':') {
        let mut split = raw.splitn(2, ':').map(str::trim);
        let (head, tail) = (split.next().unwrap(), split.next().unwrap());
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
        return vec;
    }

    vec![format!("{}\r\n", raw)]
}
