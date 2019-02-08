mod builtin;
pub use self::builtin::*;

mod shakespeare;
pub use self::shakespeare::*;

mod invest;
pub use self::invest::*;

mod twitchpoll;
pub use self::twitchpoll::*;

mod currentsong;
pub use self::currentsong::*;

mod rust;
pub use self::rust::*;

pub const MODULES: &[&str] = &[
    "Builtin",
    "CurrentSong",
    "TwitchPoll",
    "Invest",
    "RustStuff",
    "Shakespeare",
];
