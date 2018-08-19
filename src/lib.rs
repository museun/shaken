#![feature(rust_2018_preview)]
#![allow(
    unknown_lints,
    dead_code,
    unused_variables,
    unreadable_literal
)] // fuck off clippy

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

// TODO remove most of these pubs

pub mod database;

pub mod user;
pub use crate::user::*;

#[macro_use]
pub mod module;
pub use crate::module::*;

#[macro_use]
pub mod response;
pub use crate::response::*;

#[macro_use]
pub mod util;

pub mod bot;
pub use crate::bot::*;

pub mod color;

#[macro_use]
pub mod command;
pub use crate::command::*;

pub mod config;
pub use crate::config::*;

pub mod irc;

pub mod request;
pub use crate::request::*;

pub mod tags;
pub use crate::tags::*;

pub mod testing;

pub mod twitch;

pub mod modules;
pub use modules::*;
