#![feature(rust_2018_preview)]
extern crate env_logger;
#[macro_use]
extern crate log;

use std::thread;
use std::time;

extern crate shaken;
use shaken::*;

fn main() {
    env_logger::Builder::from_default_env()
        .default_format_timestamp(false)
        .init();

    let config = Config::load();
    Shaken::start(&config);
}

pub struct Shaken;

impl Shaken {
    pub fn start(config: &Config) {
        let address = format!("{}:{}", &config.twitch.address, &config.twitch.port);

        let mut sleep = 0;
        loop {
            if sleep > 0 {
                warn!("sleeping for {} seconds", sleep);
                thread::sleep(time::Duration::from_secs(sleep));
            }

            info!("trying to connect to {}", address);
            let bot = match TcpConn::new(&address) {
                Ok(conn) => {
                    sleep = 0;
                    Bot::new(conn, &config)
                }
                Err(err) => {
                    error!("error: {}", err);
                    sleep += 5;
                    continue;
                }
            };

            let _builtin = Builtin::new(&bot, &config);
            let _display = Display::new(&bot, &config);

            let _shakespeare = Shakespeare::new(&bot, &config);
            let _idlething = IdleThing::new(&bot, &config);
            let _poll = Poll::new(&bot, &config);

            info!("connected and running");
            bot.run(); // this blocks
            info!("disconnected");

            sleep += 5;
        }
    }
}
