use crate::{color::*, irc::Message as IrcMessage, tags::Kappa, *};

use crossbeam_channel as channel;
use tungstenite as ws;

use std::net::{TcpListener, TcpStream};
use std::thread;
use std::time::Duration;

#[derive(Debug, Clone, Serialize)]
struct Message {
    pub userid: String, // not sure about the lifetime yet
    pub timestamp: usize,
    pub color: RGB,
    pub display: String,
    pub data: String,
    pub kappas: Vec<Kappa>,
}

pub struct Display {
    queue: channel::Sender<Message>,
    buf: channel::Receiver<Message>,
}

impl Module for Display {
    fn command(&self, req: &Request) -> Option<Response> {
        if let Some(req) = req.search("!color") {
            return self.color_command(&req);
        }
        None
    }

    fn passive(&self, msg: &IrcMessage) -> Option<Response> {
        return self.handle_passive(msg);
    }
}

impl Default for Display {
    fn default() -> Self {
        Self::new()
    }
}

// never forget .or_else::<HashMap<String, RGB>, _>(|_: Option<()>| Ok(HashMap::new()))

impl Display {
    pub fn new() -> Self {
        let config = Config::load();

        let (tx, rx) = channel::bounded(16); // only buffer 16 messages
        Self::drain_to_client(&rx, config.websocket.address.clone());
        Self {
            queue: tx,
            buf: rx.clone(),
        }
    }

    fn color_command(&self, req: &Request) -> Option<Response> {
        let id = req.sender();
        let part = req.args().get(0)?;

        let color = RGB::from(*part);
        if color.is_dark() {
            return reply!("don't use that color");
        }

        let conn = crate::database::get_connection();
        UserStore::update_color_for_id(&conn, id, &color);
        None
    }

    fn handle_passive(&self, msg: &IrcMessage) -> Option<Response> {
        let conn = crate::database::get_connection();
        let user = UserStore::get_user_by_id(&conn, msg.tags.get_userid()?)?;

        if !msg.data.starts_with('!') {
            println!("<{}> {}", user.color.format(&user.display), &msg.data);
        }

        let ts = crate::util::get_timestamp();
        let display = Message {
            userid: user.userid.to_string(),
            timestamp: ts as usize,
            color: user.color,
            display: user.display,
            data: msg.data.clone(),
            kappas: msg.tags.get_kappas()?,
        };

        if self.queue.is_full() {
            trace!("queue is full, dropping one");
            let _ = self.buf.recv();
        }
        trace!("queue at: {}", self.queue.len());
        self.queue.send(display);

        None
    }

    fn drain_to_client(rx: &channel::Receiver<Message>, addr: String) {
        let rx = rx.clone();
        thread::spawn(move || {
            let listener = TcpListener::bind(&addr)
                .unwrap_or_else(|_| panic!("must be able to bind to {}", &addr));
            info!("websocket listening on: {}", addr);

            for stream in listener.incoming() {
                debug!("got a tcp conn for websocket");
                if let Ok(stream) = stream {
                    debug!("turned it into a websocket");
                    let rx = rx.clone();
                    Self::handle_connection(stream, &rx);
                }
            }
        });
    }

    fn handle_connection(stream: TcpStream, rx: &channel::Receiver<Message>) {
        let mut socket = match ws::accept(stream) {
            Ok(stream) => stream,
            Err(err) => {
                warn!("could not accept stream as a websocket: {}", err);
                return;
            }
        };

        // TODO make this a proper handshake
        match socket.read_message() {
            Ok(_msg) => (),
            Err(err) => {
                warn!("could not read initial message from client: {}", err);
                return;
            }
        };

        let tick = channel::tick(Duration::from_millis(1000));

        let read = |msg, socket: &mut ws::WebSocket<TcpStream>| {
            let json = serde_json::to_string(&msg).expect("well-formed structs");
            socket.write_message(ws::Message::Text(json)).is_ok()
        };

        let interval = |socket: &mut ws::WebSocket<TcpStream>| {
            let ts = crate::util::get_timestamp();
            // TODO make this less bad
            let ts = ts.to_string().as_bytes().to_vec();
            if let Err(err) = socket.write_message(ws::Message::Ping(ts)) {
                warn!("couldn't send the ping: {:?}", err);
                return false;
            }
            // is this needed?
            if let Err(err) = socket.write_pending() {
                warn!("couldn't send the ping: {:?}", err);
                return false;
            }
            if let Err(err) = socket.read_message() {
                warn!("couldn't get pong from client: {}", err);
                return false;
            }
            true
        };

        'read: loop {
            select!{
                recv(rx, msg) => { if !read(msg, &mut socket) { break 'read; } },
                recv(tick) => { if !interval(&mut socket) { break 'read; } }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use testing::*;

    #[test]
    fn color_command() {
        let display = Display::new();
        let mut env = Environment::new();
        env.add(&display);

        env.push("!color #111111");
        env.step();

        assert_eq!(env.pop(), Some("@test: don't use that color".into()));

        env.push("!color #FFFFFF");
        env.step();

        let conn = env.get_db_conn();
        let user = UserStore::get_user_by_id(&conn, 1000).unwrap();
        assert_eq!(user.color, RGB::from("#FFFFFF"));
    }
}
