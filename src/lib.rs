#![feature(rust_2018_preview)]
#![feature(unboxed_closures)]
// #![allow(unreadable_literal)]

// doesn't rust 2018 remove the need for this?
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
extern crate scoped_threadpool; // do I need this?

extern crate chrono;
extern crate curl;

extern crate rand;

extern crate ascii; // for tiny_http
extern crate tiny_http;
extern crate tungstenite;
extern crate url;

mod color;
pub use crate::color::{HSL, RGB};

mod testing;
pub use crate::testing::Environment;

mod humanize;
mod message;

#[macro_use]
mod util;

mod twitch;

mod modules;
pub use crate::modules::*;

mod bot;
pub use crate::bot::{Bot, User};

mod config;
pub use crate::config::Config;

mod conn;
pub use crate::conn::{Conn, TcpConn};
