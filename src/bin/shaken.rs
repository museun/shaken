extern crate env_logger;
#[macro_use]
extern crate log;

extern crate shaken;
use shaken::*;

fn main() {
    env_logger::Builder::from_default_env()
        .default_format_timestamp(false)
        .init();

    let config = Config::load();
    let bot = match Conn::new(&format!("{}:{}", &config.addr, &config.port)) {
        Ok(conn) => Bot::new(conn, &config),
        Err(err) => {
            error!("error: {}", err);
            ::std::process::exit(1);
        }
    };

    bot.run(&config);
}
