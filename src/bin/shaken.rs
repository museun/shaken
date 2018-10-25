use log::{error, info, warn};
use simplelog::{Config as LogConfig, LevelFilter, TermLogger};
use std::{env, thread, time};

use shaken::modules::{Builtin, Display, Invest, Shakespeare, TwitchPoll};
use shaken::prelude::*;

fn main() {
    let filter = match env::var("SHAKEN_LOG")
        .map(|s| s.to_ascii_uppercase())
        .unwrap_or_default()
        .as_str()
    {
        "TRACE" => LevelFilter::Trace,
        "DEBUG" => LevelFilter::Debug,
        "WARN" => LevelFilter::Warn,
        "ERROR" => LevelFilter::Error,

        // default
        "INFO" | _ => LevelFilter::Info,
    };

    TermLogger::init(filter, LogConfig::default()).expect("initialize logger");

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
            let conn = match irc::TcpConn::connect(&address) {
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
