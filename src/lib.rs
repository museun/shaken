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
pub(crate) use crate::user::*;

#[macro_use]
mod module;
pub use crate::module::Module;
pub(crate) use crate::module::*;

#[macro_use]
mod response;
pub(crate) use crate::response::*;

#[macro_use]
mod command;
pub(crate) use crate::command::*;

mod request;
pub(crate) use crate::request::*;

#[macro_use]
pub(crate) mod util;
#[allow(unused_imports)]
pub(crate) use crate::util::*;

pub(crate) mod tags;
pub(crate) use crate::tags::Tags;

pub(crate) mod twitch;

#[cfg(test)]
pub(crate) mod testing;
#[cfg(test)]
pub(crate) use crate::bot::ReadType;

pub mod bot;
pub use crate::bot::Bot;

pub mod modules;
// crate use modules;

pub mod database; // does this need to be public?

pub mod config;
pub use crate::config::*;

pub mod color;

mod irc;
pub use crate::irc::*;
