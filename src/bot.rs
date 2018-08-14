use crate::irc::{Conn, Message};
use crate::*;

pub struct Bot<'a> {
    conn: Conn,
    modules: Vec<&'a Box<dyn Module>>,
    // TODO this might have to be a closure
    inspect: fn(&Message, &Response),
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
            inspect: |_, _| {},
        }
    }

    pub fn add(&mut self, m: &'a Box<dyn Module>) {
        self.modules.push(m)
    }

    pub fn set_inspect(&mut self, f: fn(&Message, &Response)) {
        self.inspect = f
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
        loop {
            self.step();
        }
    }

    pub fn step(&self) {
        let msg = Message::parse(bail!(self.conn.read()).as_ref());
        let id = Self::add_user_from_msg(&msg);

        let mut out = vec![];
        let req = Request::try_parse(id, &msg.data);
        for module in &self.modules {
            match &msg.command[..] {
                "PRIVMSG" => {
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

        out.into_iter()
            .filter_map(|r| {
                r.and_then(|r| {
                    (self.inspect)(&msg, &r);
                    r.build(&msg)
                })
            }).inspect(|s| trace!("writing response: {}", s))
            .for_each(|m| self.send(&m));
    }

    fn add_user_from_msg(msg: &Message) -> i64 {
        macro_rules! expect {
            ($e:expr) => {
                $e.expect("user tags to be well formed")
            };
        };

        let user = match &msg.command[..] {
            "PRIVMSG" => Some(User {
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

        let conn = crate::database::get_connection();
        UserStore::create_user(&conn, &user)
    }
}
