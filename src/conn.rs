use std::io::{self, prelude::*, BufRead, BufReader, BufWriter, Lines};
use std::net::{self, TcpStream, ToSocketAddrs};
use std::sync::Mutex;
use std::{fmt, str};

pub enum ConnError {
    InvalidAddress(net::AddrParseError),
    CannotConnect(io::Error),
}

impl fmt::Display for ConnError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ConnError::InvalidAddress(e) => writeln!(f, "invalid address: {}", e),
            ConnError::CannotConnect(e) => writeln!(f, "cannot connect: {}", e),
        }
    }
}

pub struct Conn {
    reader: Mutex<Lines<BufReader<TcpStream>>>,
    writer: Mutex<BufWriter<TcpStream>>,
}

impl Conn {
    pub fn new<A: ToSocketAddrs>(addr: A) -> Result<Self, ConnError> {
        let conn = TcpStream::connect(&addr).map_err(ConnError::CannotConnect)?;
        debug!("connected");

        let reader = {
            let conn = conn.try_clone().expect("to clone stream");
            Mutex::new(BufReader::new(conn).lines())
        };

        let writer = {
            let conn = conn.try_clone().expect("to clone stream");
            Mutex::new(BufWriter::new(conn))
        };

        Ok(Self { reader, writer })
    }

    pub fn run(&self, process: fn(String)) {
        trace!("starting run loop");
        while let Some(line) = self.read() {
            trace!("<-- {}", line);
            process(line)
        }
        trace!("end of run loop");
    }
}

impl Proto for Conn {
    fn send(&self, data: &str) {
        // XXX: might want to rate limit here
        let mut writer = self.writer.lock().unwrap();
        for part in split(&data) {
            // don't log the password
            if &part[..4] != "PASS" {
                let line = &part[..part.len() - 2];
                trace!("--> {}", &line); // trim the \r\n
            }

            // XXX: should check to make sure its writing
            let _ = writer.write_all(part.as_bytes());
        }
        // XXX: and that its flushing
        let _ = writer.flush();
    }

    fn read(&self) -> Option<String> {
        let mut reader = self.reader.lock().unwrap();
        if let Some(Ok(line)) = reader.next() {
            trace!("<-- {}", &line);
            Some(line)
        } else {
            warn!("couldn't read line");
            None
        }
    }
}

fn split(raw: &str) -> Vec<String> {
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

// this is mostly useless for now
pub trait Proto {
    fn privmsg(&self, target: &str, data: &str) {
        debug!("> [{}]: {}", target, data);
        self.send(&format!("PRIVMSG {} :{}", target, data))
    }

    fn notice(&self, target: &str, data: &str) {
        self.send(&format!("NOTICE {} :{}", target, data))
    }

    fn join(&self, channel: &str) {
        self.send(&format!("JOIN {}", channel))
    }

    fn part(&self, channel: &str) {
        self.send(&format!("PART {}", channel))
    }

    fn read(&self) -> Option<String>;

    fn send(&self, raw: &str);
}
