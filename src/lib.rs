#![feature(rust_2018_preview)]
#![allow(dead_code, unused_variables)] // fuck off clippy

#[macro_use]
extern crate log;

extern crate serde;
#[macro_use]
extern crate serde_derive;
extern crate serde_json;
extern crate toml;

extern crate rusqlite;

#[macro_use]
extern crate crossbeam_channel;
// extern crate parking_lot;

extern crate chrono;
extern crate curl;

extern crate rand;

extern crate tungstenite;
extern crate url;

#[macro_use]
pub mod util;
pub mod color;
pub mod db;
pub mod irc;
pub mod twitch;

mod testing;

#[macro_use]
pub mod response;
pub use crate::response::*;

mod modules;
pub use crate::modules::*;

mod config;
pub use crate::config::Config;

mod tags;
pub use crate::tags::Tags;

mod bot;
pub use crate::bot::Bot;

mod command;
pub use crate::command::*;

mod module;
pub use crate::module::*;

mod request;
pub use crate::request::*;

mod user;
pub use crate::user::*;
