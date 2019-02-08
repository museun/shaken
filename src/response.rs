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

pub enum FormattedResponse {
    List(Vec<String>),
    Single(String),
}

impl From<String> for FormattedResponse {
    fn from(s: String) -> Self {
        FormattedResponse::Single(s)
    }
}

impl From<Vec<String>> for FormattedResponse {
    fn from(list: Vec<String>) -> Self {
        FormattedResponse::List(list)
    }
}

impl IntoIterator for FormattedResponse {
    type Item = String;
    type IntoIter = std::vec::IntoIter<String>;

    fn into_iter(self) -> Self::IntoIter {
        match self {
            FormattedResponse::List(list) => list.into_iter(),
            FormattedResponse::Single(item) => vec![item].into_iter(), // lame
        }
    }
}

impl Response {
    pub(crate) fn build(&self, context: Option<&irc::Message>) -> Option<FormattedResponse> {
        match self {
            Response::Multi { data } => {
                return Some(FormattedResponse::List(
                    data.iter()
                        .map(|s| s.build(context))
                        .flat_map(|s| s)
                        .flat_map(|s| s.into_iter())
                        .collect(),
                ));
            }

            Response::Command { cmd } => match cmd {
                IrcCommand::Join { channel } => return Some(format!("JOIN {}", channel).into()),
                IrcCommand::Raw { data } => return Some(data.clone().into()),
                IrcCommand::Privmsg { target, data } => {
                    return Some(format!("PRIVMSG {} :{}", target, data).into());
                }
            },

            _ => {}
        };

        let context = context.or_else(|| {
            warn!("Reply requires a message context, ignoring");
            None
        })?;

        if let Response::Action { data } = self {
            return Some(format!("PRIVMSG {} :\x01ACTION {}\x01", context.target(), data).into());
        }

        let nick = match context.prefix {
            Some(irc::Prefix::User { ref nick, .. }) => nick,
            _ => unreachable!(),
        };
        let user = UserStore::get_user_by_name(&get_connection(), &nick)?;
        match (self, context.command.as_str()) {
            (Response::Reply { data }, "PRIVMSG") => {
                Some(format!("PRIVMSG {} :@{}: {}", context.target(), user.display, data).into())
            }

            (Response::Say { data }, "PRIVMSG") => {
                Some(format!("PRIVMSG {} :{}", context.target(), data).into())
            }

            (Response::Reply { data }, "WHISPER")
            | (Response::Say { data }, "WHISPER")
            | (Response::Whisper { data }, ..) => {
                Some(format!("PRIVMSG jtv :/w {} {}", user.display, data).into())
            }
            _ => unreachable!(),
        }
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
