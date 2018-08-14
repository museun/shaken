#![feature(rust_2018_preview)]
extern crate env_logger;
#[macro_use]
extern crate log;

use std::thread;
use std::time;

extern crate shaken;
use shaken::modules::*;
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

        let builtin = &Builtin::new();
        let shakespeare = &Shakespeare::new();

        // Vec<dyn Module>
        let mods: Vec<&dyn Module> = vec![
            builtin,     //
            shakespeare, //
        ];

        let mut sleep = 0;
        loop {
            if sleep > 0 {
                warn!("sleeping for {} seconds", sleep);
                thread::sleep(time::Duration::from_secs(sleep));
            }

            info!("trying to connect to {}", address);

            // XXX: this should timeout
            let mut bot = match irc::TcpConn::new(&address) {
                Ok(conn) => {
                    sleep = 0;
                    Bot::new(conn)
                }
                Err(err) => {
                    error!("error: {}", err);
                    sleep += 5;
                    continue;
                }
            };

            for module in &mods {
                bot.add(*module);
            }

            bot.register(&config.twitch.name);

            info!("connected and running");
            bot.run(); // this blocks
            info!("disconnected");

            sleep += 5;
        }
    }
}
