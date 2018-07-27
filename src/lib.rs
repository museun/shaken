#![feature(rust_2018_preview)]
#[macro_use]
extern crate log;

extern crate serde;
#[macro_use]
extern crate serde_derive;
extern crate serde_json;
extern crate toml;

#[macro_use]
extern crate crossbeam_channel;
extern crate parking_lot;
extern crate scoped_threadpool;

extern crate chrono;
extern crate curl;
extern crate rand;
extern crate tungstenite;
extern crate url;

mod color;
mod testing;

mod humanize;
mod message;
mod util;

mod twitch;

mod modules;
pub use crate::modules::*;

mod bot;
pub use crate::bot::Bot;
mod config;
pub use crate::config::Config;
mod conn;
pub use crate::conn::{Conn, TcpConn};
