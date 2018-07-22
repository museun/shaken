#[macro_use]
extern crate log;

extern crate serde;
#[macro_use]
extern crate serde_derive;
extern crate toml;

extern crate curl;
extern crate rand;

mod bot;
pub use bot::Bot;

mod config;
pub use config::Config;

mod conn;
pub use conn::{Conn, Proto};

mod message;
