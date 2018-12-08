#![allow(clippy::unreadable_literal)]
#[macro_use]
pub mod macros;

mod bot;
mod command;
mod registry;
mod request;
mod response;
mod user;

// useful things for use outside of the bot
pub mod color;
pub mod config;
pub mod database;
pub mod irc;
pub mod module;
pub mod twitch;
pub mod util;

pub mod queue;

// actual bot modules
pub mod modules;

// testing utilities
pub(crate) mod testing;

pub mod prelude {
    pub use crate::bot::{Bot, Event, Receiver, Sender};
    pub use crate::color::{self, HSL, RGB};
    pub use crate::command::Command;
    pub use crate::config::{self, Config};
    pub use crate::database::{self, ensure_table, get_connection};
    pub use crate::irc;
    pub use crate::module::{self, CommandMap, Error as ModuleError, Module};
    pub use crate::request::Request;
    pub use crate::response::{join, multi, IrcCommand, Response};
    pub use crate::twitch::{self, TwitchClient};
    pub use crate::user::{User, UserStore};
    pub use crate::util::{self, CommaSeparated, Timestamp};

    pub use crate::registry::{
        Command as RegistryCommand,
        CommandBuilder,
        Error as RegistryError,
        Registry,
    };
}
