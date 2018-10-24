#[macro_use]
extern crate log;

use std::{thread, time};

use shaken::modules::{Builtin, Display, Invest, Shakespeare, TwitchPoll};
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

        let builtin = Builtin::new();
        let shakespeare = Shakespeare::new();
        let display = Display::new();
        let invest = Invest::new();
        let twitchpoll = TwitchPoll::new();

        let mods: Vec<&dyn Module> = vec![
            &builtin,     //
            &shakespeare, //
            &display,     //
            &invest,      //
            &twitchpoll,  //
        ];

        let mut sleep = 0;
        loop {
            if sleep > 0 {
                warn!("sleeping for {} seconds", sleep);
                thread::sleep(time::Duration::from_secs(sleep));
            }

            info!("trying to connect to {}", address);
            let conn = match TcpConn::connect(&address) {
                Ok(conn) => {
                    sleep = 0;
                    conn
                }
                Err(err) => {
                    error!("error: {}", err);
                    sleep += 5;
                    continue;
                }
            };

            let mut bot = Bot::new(conn);
            for module in &mods {
                bot.add(*module);
            }

            bot.set_inspect(|m, r| display.inspect(m, r));

            bot.register(&config.twitch.name);

            info!("connected and running");
            bot.run(); // this blocks
            info!("disconnected");

            sleep += 5;
        }
    }
}
