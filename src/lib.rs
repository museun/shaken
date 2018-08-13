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

// #[macro_use]
// extern crate crossbeam_channel;
// extern crate parking_lot;

extern crate chrono;
extern crate curl;

extern crate rand;

// extern crate tungstenite;
extern crate url;

#[macro_use]
pub mod util;
pub mod color;
pub mod db;
pub mod twitch;

mod config;
mod tags;

// mod modules;
// pub use crate::modules::*;

mod bot;
mod irc;

mod command;
mod module;
mod request;
mod response;
mod user;

pub use crate::config::Config;
pub use crate::tags::Tags;

pub use crate::bot::Bot;
// TODO don't glob these
pub use crate::command::*;
pub use crate::module::*;
pub use crate::request::*;
pub use crate::response::*;
pub use crate::user::*;
