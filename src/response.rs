use crate::irc::{Message, Prefix};
use crate::*;

#[derive(Clone, Debug, PartialEq)]
pub enum Response {
    Reply { data: String },
    Say { data: String },
    Action { data: String },
    Command { cmd: IrcCommand },
    // TODO figure out how whispers work on twitch
}

impl Response {
    pub(crate) fn build(&self, context: &Message) -> Option<String> {
        match self {
            Response::Reply { data } => {
                if let Some(Prefix::User { ref nick, .. }) = context.prefix {
                    let conn = crate::database::get_connection();
                    if let Some(user) = UserStore::get_user_by_name(&conn, &nick) {
                        Some(format!(
                            "PRIVMSG {} :@{}: {}",
                            context.target(),
                            user.display,
                            data
                        ))
                    } else {
                        // these should be panics
                        error!("user wasn't in the user store");
                        None
                    }
                } else {
                    // these should be panics
                    warn!("cannot find a prefix on that message");
                    None
                }
            }
            Response::Say { data } => Some(format!("PRIVMSG {} :{}", context.target(), data)),
            Response::Action { data } => Some(format!(
                "PRIVMSG {} :\x01ACTION {}\x01",
                context.target(),
                data
            )),
            Response::Command { cmd } => match cmd {
                IrcCommand::Join { channel } => Some(format!("JOIN {}", channel)),
                IrcCommand::Raw { data } => Some(data.clone()),
            },
        }
    }
}

#[macro_export]
macro_rules! reply {
    ($($arg:tt)*) => {
        Some(Response::Reply{data: format!($($arg)*)})
    };
}

#[macro_export]
macro_rules! say {
    ($($arg:tt)*) => {
        Some(Response::Say{data: format!($($arg)*)})
    }
}

#[macro_export]
macro_rules! action {
    ($($arg:tt)*) => {
        Some(Response::Action{data: format!($($arg)*)})
    };
}

#[derive(Clone, Debug, PartialEq)]
pub enum IrcCommand {
    Join { channel: String },
    Raw { data: String },
    // what else can we do on twitch?
}

pub fn join(ch: &str) -> Option<Response> {
    Some(Response::Command {
        cmd: IrcCommand::Join { channel: ch.into() },
    })
}

#[macro_export]
macro_rules! raw {
    ($($arg:tt)*) => {
       Some(Response::Command{cmd: $crate::IrcCommand::Raw{ data: format!($($arg)*) }})
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::irc::Message;

    fn make_test_message() -> Message {
        Message::parse(":testuser!~user@localhost PRIVMSG #test :foobar")
    }

    fn make_test_user() -> rusqlite::Connection {
        // db gets dropped
        let conn = database::get_connection();
        UserStore::create_user(
            &conn,
            &User {
                display: "TestUser".into(),
                color: color::RGB::from("#f0f0f0"),
                userid: 1004,
            },
        );
        conn
    }

    #[test]
    fn make_reply() {
        let reply = reply!("this is a {}", 42);
        assert_eq!(
            reply,
            Some(Response::Reply {
                data: "this is a 42".into()
            })
        );

        let msg = make_test_message();
        let _db = make_test_user(); // so the db doesn't get dropped before build() is called

        let output = reply.unwrap().build(&msg);
        assert_eq!(
            output,
            Some("PRIVMSG #test :@TestUser: this is a 42".into())
        );
    }

    #[test]
    fn make_say() {
        let say = say!("this is a {}", 42);
        assert_eq!(
            say,
            Some(Response::Say {
                data: "this is a 42".into()
            })
        );

        let output = say.unwrap().build(&make_test_message());
        assert_eq!(output, Some("PRIVMSG #test :this is a 42".into()));
    }

    #[test]
    fn make_action() {
        let action = action!("this is a {}", 42);
        assert_eq!(
            action,
            Some(Response::Action {
                data: "this is a 42".into()
            })
        );

        let output = action.unwrap().build(&make_test_message());
        assert_eq!(
            output,
            Some("PRIVMSG #test :\x01ACTION this is a 42\x01".into())
        );
    }

    #[test]
    fn make_join_command() {
        let join = join("#testchannel");
        assert_eq!(
            join,
            Some(Response::Command {
                cmd: IrcCommand::Join {
                    channel: "#testchannel".into()
                }
            })
        );

        let output = join.unwrap().build(&make_test_message());
        assert_eq!(output, Some("JOIN #testchannel".into()));
    }

    #[test]
    fn make_raw_command() {
        let raw = raw!("PING irc.localhost");
        assert_eq!(
            raw,
            Some(Response::Command {
                cmd: IrcCommand::Raw {
                    data: "PING irc.localhost".into()
                }
            })
        );

        let output = raw.unwrap().build(&make_test_message());
        assert_eq!(output, Some("PING irc.localhost".into()));
    }
}
