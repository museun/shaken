use crate::{bot, color::Color, config, message::Envelope};

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
    pub timestamp: String,
    pub color: Color,
    pub display: String,
    pub data: String,
}

pub struct Display {
    colors: Mutex<HashMap<String, Color>>,
    queue: channel::Sender<Message>,
}

impl Display {
    pub fn new(bot: &bot::Bot, _config: &config::Config) -> Arc<Self> {
        let colors = {
            ::std::fs::File::open("colors.json")
                .map_err(|_| None)
                .and_then(|f| {
                    serde_json::from_reader(&f).map_err(|e| {
                        error!("cannot load colors: {}", e);
                        None
                    })
                })
                .or_else::<HashMap<String, Color>, _>(|_: Option<()>| Ok(HashMap::new()))
                .unwrap()
        };

        let (tx, rx) = channel::unbounded();
        let this = Arc::new(Self {
            colors: Mutex::new(colors),
            queue: tx,
        });

        Self::drain_to_client(&rx);

        let next = Arc::clone(&this);
        bot.set_inspect(move |me, s| {
            {
                let ts = ::util::get_timestamp();
                // all this cloning
                let msg = Message {
                    userid: me.userid.to_string(),
                    timestamp: ts.to_string(),
                    color: me.color.clone(),
                    display: me.display.to_string(),
                    data: s.to_string(),
                };

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
            let badcolors = &[
                Color::from((0, 0, 0)), // black
            ];

            if let Some(id) = env.get_id() {
                if let Some(part) = env.data.split_whitespace().collect::<Vec<_>>().get(0) {
                    let color = Color::from(*part);

                    for bad in badcolors {
                        if color == *bad {
                            bot.reply(&env, "don't use that color");
                            return;
                        }
                    }
                    {
                        let mut colors = next.colors.lock();
                        colors.insert(id.to_string(), color);
                    }
                    {
                        let colors = next.colors.lock();
                        if let Ok(f) = ::std::fs::File::create("colors.json") {
                            let _ = serde_json::to_writer(&f, &*colors).map_err(|e| {
                                error!("cannot save colors: {}", e);
                            });
                        }
                    }
                }
            }
        });

        // --

        let next = Arc::clone(&this);
        bot.on_passive(move |_bot, env| {
            fn get_color_for(map: &HashMap<String, Color>, env: &'a Envelope) -> Option<Color> {
                map.get(env.get_id()?).cloned().or_else(|| {
                    env.tags
                        .get("color")
                        .and_then(|s| Some(Color::from(s)))
                        .or_else(|| Some(Color::from((255, 255, 255))))
                })
            }

            if let Some(nick) = env.get_nick() {
                trace!("tags: {:?}", env.tags);

                let color = {
                    let map = next.colors.lock();
                    get_color_for(&map, &env)
                }.unwrap();

                let display = env
                    .tags
                    .get("display-name")
                    .and_then(|s| Some(s.as_ref()))
                    .or_else(|| Some(nick))
                    .unwrap();

                {
                    let ts = ::util::get_timestamp();
                    // all this cloning
                    let msg = Message {
                        userid: env.get_id().unwrap().to_string(),
                        timestamp: ts.to_string(),
                        color: color.clone(),
                        display: display.to_string(),
                        data: env.data.to_string(),
                    };

                    next.queue.send(msg);
                }

                if env.data.starts_with('!') {
                    return;
                }
                println!("<{}> {}", color.format(&display), &env.data);
            }
        });

        this
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
            let ts = ::util::get_timestamp();
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

    fn drain_to_client(rx: &channel::Receiver<Message>) {
        // TODO get this from the config
        const ADDRESS: &str = "localhost:51000";

        let rx = rx.clone();
        thread::spawn(move || {
            let listener = TcpListener::bind(ADDRESS)
                .unwrap_or_else(|_| panic!("must be able to bind to {}", ADDRESS));

            for stream in listener.incoming() {
                if let Ok(stream) = stream {
                    let rx = rx.clone();
                    Self::handle_connection(stream, &rx);
                }
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use testing::*;

    #[test]
    fn test_serialize() {
        init_logger();

        let ts = ::util::get_timestamp();

        let data = Message::Display {
            userid: "1000".to_string(),
            timestamp: ts.to_string(),
            color: Color::from("#FF00FF"),
            display: "test user".to_string(),
            data: "this is a test message".to_string(),
        };

        info!("{}", serde_json::to_string_pretty(&data).unwrap());
    }
}
