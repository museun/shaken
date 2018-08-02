use crate::{bot::Bot, color::RGB, config, message::Envelope};

use crossbeam_channel as channel;
use parking_lot::Mutex;
use std::collections::HashMap;
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::thread;
use std::time::Duration;
use tungstenite as ws;

#[derive(Debug, Clone, Serialize)]
struct Message {
    pub userid: String, // not sure about the lifetime yet
    pub timestamp: usize,
    pub color: RGB,
    pub display: String,
    pub data: String,
}

pub struct Display {
    colors: Mutex<HashMap<String, RGB>>,
    queue: channel::Sender<Message>,
    buf: channel::Receiver<Message>,
}

impl Display {
    pub fn new(bot: &Bot, config: &config::Config) -> Arc<Self> {
        let colors = {
            ::std::fs::File::open("colors.json")
                .map_err(|_| None)
                .and_then(|f| {
                    serde_json::from_reader(&f).map_err(|e| {
                        error!("cannot load colors: {}", e);
                        None
                    })
                }).or_else::<HashMap<String, RGB>, _>(|_: Option<()>| Ok(HashMap::new()))
                .unwrap()
        };

        let (tx, rx) = channel::bounded(16); // only buffer 16 messages

        let this = Arc::new(Self {
            colors: Mutex::new(colors),
            queue: tx,
            buf: rx.clone(),
        });

        Self::drain_to_client(&rx, config.websocket.address.clone());

        let next = Arc::clone(&this);
        bot.set_inspect(move |me, s| {
            {
                let ts = crate::util::get_timestamp();
                // all this cloning
                let msg = Message {
                    userid: me.userid.to_string(),
                    timestamp: ts as usize,
                    color: me.color.clone(),
                    display: me.display.to_string(),
                    data: s.to_string(),
                };

                if next.queue.is_full() {
                    debug!("queue is full, dropping one");
                    let _ = next.buf.recv();
                }
                debug!("queue at: {}", next.queue.len());
                next.queue.send(msg);
            }

            // disable @mention display
            if s.starts_with('@') {
                return;
            }

            let display = me.color.format(&me.display);
            println!("<{}> {}", &display, &s)
        });

        let next = Arc::clone(&this);
        bot.on_command("!color", move |bot, env| {
            next.color_command(bot, env);
        });

        // --

        let next = Arc::clone(&this);
        bot.on_passive(move |bot, env| {
            next.handle_passive(bot, env);
        });

        this
    }

    fn handle_passive(&self, _bot: &Bot, env: &Envelope) -> Option<()> {
        fn get_color_for(map: &HashMap<String, RGB>, env: &'a Envelope) -> Option<RGB> {
            map.get(env.get_id()?).cloned().or_else(|| {
                env.tags
                    .get("color")
                    .and_then(|s| Some(RGB::from(s)))
                    .or_else(|| Some(RGB::from((255, 255, 255))))
            })
        }

        let nick = env.get_nick()?;
        trace!("tags: {:?}", env.tags);

        let color = {
            let map = self.colors.lock();
            get_color_for(&map, &env)
        }?;

        let display = env
            .tags
            .get("display-name")
            .and_then(|s| Some(s.as_ref()))
            .or_else(|| Some(nick))?;

        {
            let ts = crate::util::get_timestamp();
            // all this cloning
            let msg = Message {
                userid: env.get_id().unwrap().to_string(),
                timestamp: ts as usize,
                color: color.clone(),
                display: display.to_string(),
                data: env.data.to_string(),
            };

            if self.queue.is_full() {
                debug!("queue is full, dropping one");
                let _ = self.buf.recv();
            }
            debug!("queue at: {}", self.queue.len());
            self.queue.send(msg);
        }

        if env.data.starts_with('!') {
            return None;
        }
        println!("<{}> {}", color.format(&display), &env.data);

        None
    }

    fn color_command(&self, bot: &Bot, env: &Envelope) -> Option<()> {
        let id = env.get_id()?;

        let parts = env.data.split_whitespace().collect::<Vec<_>>();
        let part = parts.get(0)?;

        let color = RGB::from(*part);
        if color.is_dark() {
            bot.reply(&env, "don't use that color");
            return None;
        }

        {
            let mut colors = self.colors.lock();
            colors.insert(id.to_string(), color);
        }
        {
            let colors = self.colors.lock();
            if let Ok(f) = ::std::fs::File::create("colors.json") {
                let _ = serde_json::to_writer(&f, &*colors).map_err(|e| {
                    error!("cannot save colors: {}", e);
                });
            }
        }

        None
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
}
