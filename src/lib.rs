#![feature(rust_2018_preview)]
//#![allow(dead_code, unused_variables)] // fuck off clippy

#[macro_use]
extern crate log;
extern crate serde;
#[macro_use]
extern crate serde_derive;
extern crate rusqlite;
extern crate serde_json;
extern crate toml;
#[macro_use]
extern crate crossbeam_channel;
extern crate chrono;
extern crate curl;
extern crate parking_lot;
extern crate rand;
extern crate tungstenite;
extern crate url;

pub mod database;
pub mod user;
pub use user::*;
pub mod module;
pub use module::*;

#[macro_use]
pub mod response;
#[macro_use]
pub mod util;

pub mod bot;
pub mod color;
pub mod command;
pub mod config;
pub mod irc;
pub mod request;
pub mod tags;
pub mod testing;
pub mod twitch;

pub use bot::Bot;
pub use command::*;
pub use config::Config;
pub use modules::*;
pub use request::*;
pub use response::*;
pub use tags::Tags;

pub mod modules;
