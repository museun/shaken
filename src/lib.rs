#![feature(rust_2018_preview)]
#[macro_use]
extern crate log;

extern crate serde;
#[macro_use]
extern crate serde_derive;
extern crate toml;

extern crate curl;
extern crate rand;

mod bot;
pub use crate::bot::Bot;

mod config;
pub use crate::config::Config;

mod conn;
pub use crate::conn::{Conn, Proto};

mod message;

mod state;
