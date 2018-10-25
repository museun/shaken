#![allow(clippy::unreadable_literal)]
#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate log;
#[macro_use]
extern crate crossbeam_channel;

#[macro_use]
pub mod macros;

mod bot;
mod command;

mod module;
mod request;
mod response;
mod user;

// useful things for use outside of the bot
pub mod color;
pub mod config;
pub mod database;
pub mod irc;
pub mod twitch;
pub mod util;

// actual bot modules
pub mod modules;

// testing utilities
pub(crate) mod testing;

pub mod prelude {
    pub use crate::bot::{Bot, ReadType};
    pub use crate::color::{self, HSL, RGB};
    pub use crate::command::Command;
    pub use crate::config::{self, Config, Invest, Shakespeare, Twitch, WebSocket};
    pub use crate::database::{self, ensure_table, get_connection};
    pub use crate::irc::{self, Conn};
    pub use crate::module::{Every, Module};
    pub use crate::request::Request;
    pub use crate::response::{join, multi, IrcCommand, Response};
    pub use crate::twitch::{self, TwitchClient};
    pub use crate::user::{User, UserStore};
    pub use crate::util::{self, CommaSeparated, Timestamp};
}
