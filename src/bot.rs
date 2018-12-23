use crate::prelude::*;
use crossbeam_channel as channel;
use log::*;
use std::thread;
use std::time::{Duration, Instant};

pub type Receiver = channel::Receiver<Event>;
pub type Sender = channel::Sender<(Option<irc::Message>, Response)>;

#[derive(Debug, Clone)]
pub enum Event {
    Message(irc::Message, Option<Box<Request>>),
    Inspect(irc::Message, Box<Response>),
    Tick(Instant),
}

pub struct Bot {
    out_tx: channel::Sender<String>,
    inspect_tx: channel::Sender<(irc::Message, Box<Response>)>,
}

impl Bot {
    pub fn create(mut conn: irc::TcpConn) -> (Self, Receiver) {
        let (in_tx, in_rx) = channel::unbounded();
        let (out_tx, out_rx) = channel::unbounded::<String>();
        let (inspect_tx, inspect_rx) = channel::bounded(4);

        thread::spawn(move || {
            let tick = channel::tick(Duration::from_millis(1000));

            loop {
                if let Ok(data) = out_rx.try_recv() {
                    conn.write(&data)
                }
                match conn.try_read() {
                    Some(irc::ReadStatus::Data(msg)) => {
                        let msg = irc::Message::parse(&msg);
                        let req = Request::try_from(&msg);
                        in_tx.send(Event::Message(msg, req.map(Box::new)));
                    }
                    Some(irc::ReadStatus::Nothing) => {}
                    _ => {
                        drop(in_tx);
                        return;
                    }
                };

                if let Ok(tick) = tick.try_recv() {
                    in_tx.send(Event::Tick(tick));
                }

                if let Ok((msg, resp)) = inspect_rx.try_recv() {
                    in_tx.send(Event::Inspect(msg, resp));
                }
            }
        });

        (Bot { out_tx, inspect_tx }, in_rx)
    }

    pub fn send(&self, data: impl Into<String>) {
        self.out_tx.send(data.into());
    }

    pub fn process(&self, rx: channel::Receiver<(Option<irc::Message>, Response)>) {
        for (msg, resp) in rx {
            let msg = msg.as_ref();
            if let Some(msg) = msg {
                self.inspect_tx.send((msg.clone(), Box::new(resp.clone())));
            }

            if let Some(resp) = resp.build(msg) {
                resp.into_iter()
                    .inspect(|s| trace!("writing response: {}", s))
                    .for_each(|m| self.send(m));
            }
        }
    }

    pub fn register(&self, nick: &str) {
        trace!("registering");

        // ircv3 stuff
        self.send("CAP REQ :twitch.tv/tags");
        self.send("CAP REQ :twitch.tv/membership");
        self.send("CAP REQ :twitch.tv/commands");

        let password = match std::env::var("SHAKEN_TWITCH_PASSWORD") {
            Ok(password) => password,
            Err(_err) => {
                error!("env variable must be set: SHAKEN_TWITCH_PASSWORD");
                std::process::exit(1);
            }
        };

        self.send(format!("PASS {}", password));
        self.send(format!("NICK {}", &nick));

        // this would be needed for a real irc server
        // self.send(&format!("USER {} * 8 :{}", "shaken", "shaken bot"));

        trace!("registered");
    }
}
