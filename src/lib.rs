#![allow(clippy::unreadable_literal)]

#[macro_use]
extern crate log;
#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate crossbeam_channel;

// TODO go thru this
// TODO make a prelude

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

pub(crate) mod tags;
pub(crate) use crate::tags::Tags;

pub(crate) mod twitch;

pub(crate) mod testing;
pub(crate) use crate::bot::ReadType;

pub mod bot;
pub use crate::bot::Bot;

pub mod modules;

pub mod database; // does this need to be public?

pub mod config;
pub use crate::config::*;

pub mod color;

mod irc;
pub use crate::irc::*;
