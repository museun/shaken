use crate::irc::{Conn, Message, Prefix};
use crate::*;

use crossbeam_channel as channel;
use parking_lot::Mutex;

use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

pub struct Bot<'a, T>
where
    T: Conn + 'static,
{
    conn: Arc<Mutex<Connection<T>>>,
    modules: Vec<&'a dyn Module>,
    inspect: Box<Fn(&Message, &Response) + 'a>,
}

impl<'a, T> Bot<'a, T>
where
    T: Conn + 'static,
{
    /// just clone the connection
    pub fn new(conn: T) -> Self {
        let conn = Connection::new(conn);

        Self {
            conn: Arc::new(Mutex::new(conn)),
            modules: vec![],
            inspect: Box::new(|_, _| {}),
        }
    }

    pub(crate) fn get_conn_mut(&mut self) -> Arc<Mutex<Connection<T>>> {
        Arc::clone(&self.conn)
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
        // self.send(&format!("USER {} * 8 :{}", "shaken", "shaken bot"));

        trace!("registered");
    }

    pub fn send(&self, data: &str) {
        self.conn.lock().write(data);
    }

    pub fn run(&self) {
        trace!("starting run loop");

        let (quittx, quitrx): (channel::Sender<()>, channel::Receiver<()>) = channel::bounded(0);

        let (tx, rx) = channel::bounded(10);
        let out = tx.clone();
        let quit = quitrx.clone();
        thread::spawn(move || {
            let ticker = channel::tick(Duration::from_millis(1000));
            loop {
                select! {
                    recv(ticker, _) => {
                        out.send(ReadType::Tick(Instant::now()));
                    },
                    recv(quit, _) => {
                        break;
                    }
                }
            }
        });

        let out = tx.clone();
        let quit = quittx.clone();
        let conn = Arc::clone(&self.conn);
        thread::spawn(move || loop {
            if let Some(ref mut conn) = conn.try_lock_for(Duration::from_millis(50)) {
                if let Some(msg) = conn.try_read() {
                    if let Some(msg) = msg {
                        out.send(ReadType::Message(msg));
                    }
                } else {
                    quit.send(());
                    return;
                }
            }
        });

        let rx = rx.clone();
        while let Some(_) = { rx.recv().and_then(|next| self.step(&next)) } {}
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

    pub fn step(&self, next: &ReadType) -> Option<()> {
        let mut out = vec![];
        if let ReadType::Message(..) = next {
            trace!("dispatching to modules");
        }

        let ctx = match next {
            ReadType::Message(data) => {
                trace!("handling message");
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
                    out.push(module.tick(*dt))
                }
                None
            }
        };

        if let ReadType::Message(..) = next {
            trace!("done dispatching to modules");
        }

        if let ReadType::Message(..) = next {
            trace!("collecting to send");
        }

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

        if let ReadType::Message(..) = next {
            trace!("done sending");
        }

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
        }
        .unwrap();

        trace!("trying to add user {:?}", user);
        let conn = crate::database::get_connection();
        let id = UserStore::create_user(&conn, &user);
        trace!("added user {:?}: {}", user, id);
        id
    }
}

pub enum ReadType {
    Tick(Instant),
    Message(String),
}
