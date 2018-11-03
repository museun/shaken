use crate::prelude::*;

#[derive(Clone, Debug, PartialEq)]
pub enum Response {
    Multi { data: Vec<Box<Response>> },
    Reply { data: String },
    Say { data: String },
    Action { data: String },
    Whisper { data: String },
    Command { cmd: IrcCommand },
}

impl Response {
    pub(crate) fn build(&self, context: Option<&irc::Message>) -> Option<Vec<String>> {
        match self {
            Response::Reply { data } => {
                let context = context.or_else(|| {
                    warn!("Reply requires a message context, ignoring");
                    None
                })?;
                let nick = match context.prefix {
                    Some(irc::Prefix::User { ref nick, .. }) => nick,
                    _ => unreachable!(),
                };
                let user = UserStore::get_user_by_name(&get_connection(), &nick)?;
                match context.command.as_str() {
                    "PRIVMSG" => {
                        return Some(vec![format!(
                            "PRIVMSG {} :@{}: {}",
                            context.target(),
                            user.display,
                            data
                        )])
                    }
                    "WHISPER" => {
                        return Some(vec![format!("PRIVMSG jtv :/w {} {}", user.display, data)])
                    }
                    _ => unreachable!(),
                }
            }
            Response::Say { data } => {
                let context = context.or_else(|| {
                    warn!("Say requires a message context, ignoring");
                    None
                })?;
                let nick = match context.prefix {
                    Some(irc::Prefix::User { ref nick, .. }) => nick,
                    _ => unreachable!(),
                };
                let user = UserStore::get_user_by_name(&get_connection(), &nick)?;
                match context.command.as_str() {
                    "PRIVMSG" => {
                        return Some(vec![format!("PRIVMSG {} :{}", context.target(), data)])
                    }
                    "WHISPER" => {
                        return Some(vec![format!("PRIVMSG jtv :/w {} {}", user.display, data)])
                    }
                    _ => unreachable!(),
                }
            }
            Response::Action { data } => {
                let context = context.or_else(|| {
                    warn!("Action requires a message context, ignoring");
                    None
                })?;
                return Some(vec![format!(
                    "PRIVMSG {} :\x01ACTION {}\x01",
                    context.target(),
                    data
                )]);
            }
            Response::Whisper { data } => {
                let context = context.or_else(|| {
                    warn!("Whisper requires a message context, ignoring");
                    None
                })?;
                if let Some(irc::Prefix::User { ref nick, .. }) = context.prefix {
                    let conn = crate::database::get_connection();
                    if let Some(user) = UserStore::get_user_by_name(&conn, &nick) {
                        return Some(vec![format!("PRIVMSG jtv :/w {} {}", user.display, data)]);
                    }
                }
            }
            Response::Multi { data } => {
                return Some(
                    data.iter()
                        .map(|s| s.build(context))
                        .flat_map(|s| s)
                        .flat_map(|s| s.into_iter())
                        .collect(),
                );
            }
            Response::Command { cmd } => match cmd {
                IrcCommand::Join { channel } => return Some(vec![format!("JOIN {}", channel)]),
                IrcCommand::Raw { data } => return Some(vec![data.clone()]),
                IrcCommand::Privmsg { target, data } => {
                    return Some(vec![format!("PRIVMSG {} :{}", target, data)])
                }
            },
        }

        panic!("invalid bot state");
    }
}

pub fn multi(iter: impl Iterator<Item = Option<Response>>) -> Option<Response> {
    Some(Response::Multi {
        data: iter.filter_map(|s| s).map(Box::new).collect(),
    })
}

#[derive(Clone, Debug, PartialEq)]
pub enum IrcCommand {
    Join { channel: String },
    Raw { data: String },
    Privmsg { target: String, data: String },
    // what else can we do on twitch?
}

pub fn join(ch: &str) -> Option<Response> {
    Some(Response::Command {
        cmd: IrcCommand::Join { channel: ch.into() },
    })
}

/*
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{irc::Message, testing::*};

    fn make_test_message() -> Message {
        Message::parse(":testuser!~user@localhost PRIVMSG #test :foobar")
    }

    fn make_test_user() {
        let conn = database::get_connection();
        UserStore::create_user(
            &conn,
            &User {
                display: "TestUser".into(),
                color: color::RGB::from("#f0f0f0"),
                userid: 1004,
            },
        );
    }

    #[test]
    fn make_reply() {
        let _env = Environment::new();

        let reply = reply!("this is a {}", 42);
        assert_eq!(
            reply,
            Some(Response::Reply {
                data: "this is a 42".into()
            })
        );

        let msg = Some(make_test_message());
        make_test_user();

        let output = reply.unwrap().build(msg.as_ref());
        assert_eq!(
            output,
            vec!["PRIVMSG #test :@TestUser: this is a 42".to_owned()]
        );
    }

    #[test]
    fn make_whisper() {
        let env = Environment::new();

        let whisper = whisper!("this is a {}", 42);
        assert_eq!(
            whisper,
            Some(Response::Whisper {
                data: "this is a 42".into()
            })
        );

        let mut msg = make_test_message();
        msg.command = "WHISPER".into();
        msg.args[0] = env.get_user_name().into();

        make_test_user();

        let output = whisper.unwrap().build(Some(&msg));
        assert_eq!(
            output,
            vec!["PRIVMSG jtv :/w TestUser this is a 42".to_owned()]
        );
    }

    #[test]
    fn make_say() {
        let _env = Environment::new();

        let say = say!("this is a {}", 42);
        assert_eq!(
            say,
            Some(Response::Say {
                data: "this is a 42".into()
            })
        );

        let output = say.unwrap().build(Some(&make_test_message()));
        assert_eq!(output, vec!["PRIVMSG #test :this is a 42".to_owned()]);
    }

    #[test]
    fn make_action() {
        let _env = Environment::new();

        let action = action!("this is a {}", 42);
        assert_eq!(
            action,
            Some(Response::Action {
                data: "this is a 42".into()
            })
        );

        let output = action.unwrap().build(Some(&make_test_message()));
        assert_eq!(
            output,
            vec!["PRIVMSG #test :\x01ACTION this is a 42\x01".to_owned()]
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

        let output = join.unwrap().build(None);
        assert_eq!(output, vec!["JOIN #testchannel".to_owned()]);
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

        let output = raw.unwrap().build(None);
        assert_eq!(output, vec!["PING irc.localhost".to_owned()]);
    }

    #[test]
    fn make_privmsg_command() {
        let privmsg = privmsg!("#test", "this is a {}", 42);
        assert_eq!(
            privmsg,
            Some(Response::Command {
                cmd: IrcCommand::Privmsg {
                    target: "#test".into(),
                    data: "this is a 42".into()
                }
            })
        );

        let output = privmsg.unwrap().build(None);
        assert_eq!(output, vec!["PRIVMSG #test :this is a 42".to_owned()]);
    }

    #[test]
    fn make_multi() {
        let _env = Environment::new();
        make_test_user();

        let resp = multi!(
            reply!("hello"),
            say!("test"),
            None,
            raw!("PING irc.localhost"),
            join("#foobar"),
            None,
            multi!(reply!("a"), reply!("b"),),
            None,
            multi((0..3).map(|s| say!("{}", s.to_string())))
        );

        assert_eq!(
            resp,
            Some(Response::Multi {
                data: vec![
                    Box::new(Response::Reply {
                        data: "hello".into()
                    }),
                    Box::new(Response::Say {
                        data: "test".into()
                    }),
                    Box::new(Response::Command {
                        cmd: IrcCommand::Raw {
                            data: "PING irc.localhost".into()
                        }
                    }),
                    Box::new(Response::Command {
                        cmd: IrcCommand::Join {
                            channel: "#foobar".into()
                        }
                    }),
                    Box::new(Response::Multi {
                        data: vec![
                            Box::new(Response::Reply { data: "a".into() }),
                            Box::new(Response::Reply { data: "b".into() })
                        ]
                    }),
                    Box::new(Response::Multi {
                        data: vec![
                            Box::new(Response::Say { data: "0".into() }),
                            Box::new(Response::Say { data: "1".into() }),
                            Box::new(Response::Say { data: "2".into() })
                        ]
                    }),
                ]
            })
        );

        let out = resp.unwrap().build(Some(&make_test_message()));
        assert_eq!(
            out,
            vec![
                "PRIVMSG #test :@TestUser: hello".to_owned(),
                "PRIVMSG #test :test".to_owned(),
                "PING irc.localhost".to_owned(),
                "JOIN #foobar".to_owned(),
                "PRIVMSG #test :@TestUser: a".to_owned(),
                "PRIVMSG #test :@TestUser: b".to_owned(),
                "PRIVMSG #test :0".to_owned(),
                "PRIVMSG #test :1".to_owned(),
                "PRIVMSG #test :2".to_owned(),
            ]
        );
    }
}
*/
