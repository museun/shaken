use crate::prelude::*;
use log::*;

#[derive(Clone, Debug, PartialEq)]
pub enum Response {
    Multi { data: Vec<Response> },
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
                        )]);
                    }
                    "WHISPER" => {
                        return Some(vec![format!("PRIVMSG jtv :/w {} {}", user.display, data)]);
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
                        return Some(vec![format!("PRIVMSG {} :{}", context.target(), data)]);
                    }
                    "WHISPER" => {
                        return Some(vec![format!("PRIVMSG jtv :/w {} {}", user.display, data)]);
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
                    return Some(vec![format!("PRIVMSG {} :{}", target, data)]);
                }
            },
        }

        unreachable!()
    }
}

pub fn multi(iter: impl Iterator<Item = Option<Response>>) -> Option<Response> {
    Some(Response::Multi {
        data: iter.filter_map(|s| s).collect(),
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
