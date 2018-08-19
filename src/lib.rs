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

mod user;
crate use crate::user::*;

#[macro_use]
mod module;
pub use crate::module::Module;
crate use crate::module::*;

#[macro_use]
mod response;
crate use crate::response::*;
#[macro_use]
mod command;
crate use crate::command::*;
mod request;
crate use crate::request::*;

#[macro_use]
crate mod util;

crate mod tags;
crate use crate::tags::Tags;

#[cfg(test)]
crate mod testing;
crate mod twitch;

pub mod modules;
pub use modules::*;

pub mod bot;
pub mod database;
pub use crate::bot::Bot;
pub mod config;
pub use crate::config::*;
pub mod color;
pub mod irc;
