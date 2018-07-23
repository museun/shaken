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
    let addr = format!("{}:{}", &config.addr, &config.port);

    let mut sleep = 0;
    loop {
        thread::sleep(time::Duration::from_secs(sleep));

        info!("trying to connect to {}", addr);
        let bot = match Conn::new(&addr) {
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

        bot.run(&config); // this blocks
        info!("disconnected");

        sleep += 5;
    }
}
