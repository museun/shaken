use crate::irc::{Conn, Message, Prefix};
use crate::*;

use crossbeam_channel as channel;
use std::thread;
use std::time::{Duration, Instant};

pub struct Bot<'a> {
    conn: Conn,
    modules: Vec<&'a dyn Module>,
    // TODO this might have to be a closure
    inspect: Box<Fn(&Message, &Response) + 'a>,
}

impl<'a> Bot<'a> {
    /// just clone the connection
    pub fn new<C>(conn: C) -> Self
    where
        C: Into<Conn>,
    {
        let conn = conn.into();
        Self {
            conn,
            modules: vec![],
            inspect: Box::new(|_, _| {}),
        }
    }

    pub fn add(&mut self, m: &'a dyn Module) {
        self.modules.push(m)
    }

    pub fn set_inspect<F>(&mut self, f: F)
    where
        F: Fn(&Message, &Response) + 'a,
    {
        self.inspect = Box::new(f)
    }

    pub fn register(&self, nick: &str) {
        trace!("registering");

        // ircv3 stuff
        self.send("CAP REQ :twitch.tv/tags");
        self.send("CAP REQ :twitch.tv/membership");
        self.send("CAP REQ :twitch.tv/commands");

        self.send(&format!("PASS {}", env!("SHAKEN_TWITCH_PASSWORD")));
        self.send(&format!("NICK {}", &nick));

        // this would be needed for a real irc server
        // self.conn
        //     .write(&format!("USER {} * 8 :{}", "shaken_bot", "shaken_bot"));

        trace!("registered");
    }

    pub fn send(&self, data: &str) {
        self.conn.write(data)
    }

    pub fn run(&self) {
        trace!("starting run loop");

        let (tx, rx) = channel::bounded(1);
        let tx = tx.clone();
        thread::spawn(move || {
            let after = channel::after(Duration::from_millis(1000));
            for dt in after {
                tx.send(dt);
            }
        });

        while let Some(_) = {
            let rx = rx.clone();
            self.step(&rx)
        } {}
        trace!("ending the run loop");
    }

    fn try_make_request(msg: &Message) -> Option<Request> {
        let id = Self::add_user_from_msg(&msg);
        trace!("trying to make request for: `{}` {:?}", id, msg);
        match &msg.command[..] {
            "PRIVMSG" | "WHISPER" => {
                // sanity check
                if let Some(Prefix::User { .. }) = msg.prefix {
                    // HACK: this is ugly
                    Request::try_parse(msg.target(), id, &msg.data)
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    pub fn step(&self, tick: &channel::Receiver<Instant>) -> Option<()> {
        let ty = select! {
            recv(tick, dt) => {
                if let Some(dt) = dt {
                    ReadType::Tick(dt)
                } else {
                    return None;
                }
            }
            default => {
                ReadType::Message(self.conn.read()?)
            }
        };

        let mut out = vec![];
        trace!("dispatching to modules");
        let ctx = match ty {
            ReadType::Message(data) => {
                let msg = Message::parse(&data);
                let req = Self::try_make_request(&msg);

                for module in &self.modules {
                    match &msg.command[..] {
                        "PRIVMSG" | "WHISPER" => {
                            // try commands first
                            if let Some(req) = &req {
                                out.push(module.command(req))
                            }
                            // then passives
                            out.push(module.passive(&msg));
                        }
                        // other message types go to the event handler
                        _ => out.push(module.event(&msg)),
                    }
                }

                Some(msg.clone()) // whatever
            }
            ReadType::Tick(dt) => {
                for module in &self.modules {
                    out.push(module.tick(dt))
                }
                None
            }
        };
        trace!("done dispatching to modules");

        trace!("collecting to send");
        for resp in out.into_iter().filter_map(|s| s) {
            let ctx = ctx.as_ref();
            if ctx.is_some() {
                (self.inspect)(ctx.unwrap(), &resp);
            }

            resp.build(ctx)
                .into_iter()
                .inspect(|s| trace!("writing response: {}", s))
                .for_each(|m| self.send(&m));
        }
        trace!("done sending");
        Some(())
    }

    fn add_user_from_msg(msg: &Message) -> i64 {
        macro_rules! expect {
            ($e:expr) => {
                $e.expect("user tags to be well formed")
            };
        };

        let user = match &msg.command[..] {
            "PRIVMSG" | "WHISPER" => Some(User {
                display: expect!(msg.tags.get_display()).to_string(),
                color: expect!(msg.tags.get_color()),
                userid: expect!(msg.tags.get_userid()),
            }),
            // this is /our/ user
            "GLOBALUSERSTATE" => Some(User {
                display: expect!(msg.tags.get_display()).to_string(),
                color: color::RGB::from("fc0fc0"),
                userid: expect!(msg.tags.get_userid()),
            }),
            _ => return -1,
        }.unwrap();

        trace!("trying to add user {:?}", user);
        let conn = crate::database::get_connection();
        let id = UserStore::create_user(&conn, &user);
        trace!("added user {:?}: {}", user, id);
        id
    }
}

enum ReadType {
    Tick(Instant),
    Message(String),
}
