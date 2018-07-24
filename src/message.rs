use std::collections::HashMap;
use std::fmt;

#[derive(Debug, Clone)]
pub struct Envelope {
    pub channel: String,
    pub sender: Prefix,
    pub data: String,
    pub tags: HashMap<String, String>,
}

impl Envelope {
    pub fn from_msg(msg: &Message) -> Self {
        assert!(msg.command == "PRIVMSG");

        let msg = msg.clone();
        Self {
            channel: msg.args[0].to_string(),
            sender: msg.prefix.unwrap(),
            data: msg.data.to_string(),
            tags: msg.tags,
        }
    }

    pub fn get_nick(&self) -> Option<&str> {
        if let Prefix::User { ref nick, .. } = self.sender {
            trace!("got nick: {}", nick);
            return Some(nick);
        }

        warn!("no nick attached to that message");
        None
    }
}

// make sure it has caps before sending to this function
fn parse_tags(s: &str) -> (&str, HashMap<String, String>) {
    let n = s.find(' ').unwrap();
    let sub = &s[..n];
    let mut map = HashMap::new();
    for part in sub.split_terminator(';') {
        if let Some(index) = part.find('=') {
            let (k, v) = (&part[..index], &part[index + 1..]);
            map.insert(k.into(), v.into());
        }
    }
    (&s[n + 1..], map)
}

#[derive(Debug, PartialEq, Clone)]
pub struct Message {
    pub tags: HashMap<String, String>,
    pub prefix: Option<Prefix>,
    pub command: String,
    pub args: Vec<String>,
    pub data: String,
}

impl Message {
    // should probably return a result
    pub fn parse(input: &str) -> Message {
        let (input, tags) = if !input.starts_with(':') && !input.starts_with("PING") {
            parse_tags(&input)
        } else {
            (input, HashMap::new())
        };

        let prefix = Prefix::parse(&input);

        let iter = input
            .split_whitespace()
            .skip(if prefix.is_some() { 1 } else { 0 });

        let mut args = iter
            .take_while(|s| !s.starts_with(':'))
            .map(|s| s.into())
            .collect::<Vec<_>>();
        let command = args.remove(0);

        let data = if let Some(pos) = &input[1..].find(':') {
            input[*pos + 2..].into()
        } else {
            "".into()
        };

        Self {
            tags,
            prefix,
            command,
            args,
            data,
        }
    }
}

impl fmt::Display for Message {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let prefix = match &self.prefix {
            Some(Prefix::User { nick, .. }) => nick.trim(),
            Some(Prefix::Server { host, .. }) => host.trim(),
            None => "??",
        };

        let data = self.data.trim();

        match self.command.as_ref() {
            "PRIVMSG" => write!(f, "< [{}] <{}> {}", self.args[0], prefix, data),
            _ => write!(
                f,
                "({}) <{}> {:?}: {}",
                &self.command, prefix, self.args, data
            ),
        }
    }
}

#[derive(Debug, PartialEq, Clone)]
pub enum Prefix {
    User {
        nick: String,
        user: String,
        host: String,
    },
    Server {
        host: String,
    },
}

impl Prefix {
    pub fn parse(input: &str) -> Option<Self> {
        if !input.starts_with(':') {
            // XXX: will this be a problem?
            None?;
        }

        let s = input[1..input.find(' ')?].trim();
        match s.find('!') {
            Some(pos) => {
                let nick = &s[..pos];
                let at = s.find('@')?;
                let user = &s[pos + 1..at];
                let host = &s[at + 1..];
                Some(Prefix::User {
                    nick: nick.into(),
                    user: user.into(),
                    host: host.into(),
                })
            }
            None => Some(Prefix::Server { host: s.into() }),
        }
    }
}

impl fmt::Display for Prefix {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Prefix::User {
                ref nick,
                ref user,
                ref host,
            } => writeln!(f, "{}!{}@{}", nick, user, host),
            Prefix::Server { ref host } => writeln!(f, "{}", host),
        }
    }
}

/*
#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn parse_user_prefix() {
        let prefix = Prefix::parse(":test!some@user.localhost 001 welcome to irc");
        assert!(prefix.is_some(), "didn't parse prefix");
        let prefix = prefix.unwrap();
        assert_eq!(
            prefix,
            Prefix::User {
                nick: "test".into(),
                user: "some".into(),
                host: "user.localhost".into()
            },
        )
    }

    #[test]
    fn parse_server_prefix() {
        let prefix = Prefix::parse(":irc.localhost 001 welcome to irc");
        assert!(prefix.is_some(), "didn't parse prefix");
        let prefix = prefix.unwrap();
        assert_eq!(
            prefix,
            Prefix::Server {
                host: "irc.localhost".into()
            },
        )
    }

    #[test]
    fn parse_message() {
        let input = ":test.localhost 001 museun :Welcome to the Internet Relay Network museun";
        let msg = Message::parse(&input);
        assert_eq!(
            msg,
            Message {
                prefix: Some(Prefix::Server {
                    host: "test.localhost".into()
                }),
                command: "001".into(),
                args: vec!["museun".into()],
                data: "Welcome to the Internet Relay Network museun".into(),
            },
        );

        let input = ":museun!~museun@test.localhost JOIN :#test";
        let msg = Message::parse(&input);
        assert_eq!(
            msg,
            Message {
                prefix: Some(Prefix::User {
                    nick: "museun".into(),
                    user: "~museun".into(),
                    host: "test.localhost".into()
                }),
                command: "JOIN".into(),
                args: vec![],
                data: "#test".into(),
            },
        );

        let input = ":test.localhost 354 museun 152 #test ~museun test.localhost test.localhost museun H@ 0 :realname";
        let msg = Message::parse(&input);
        assert_eq!(
            msg,
            Message {
                prefix: Some(Prefix::Server {
                    host: "test.localhost".into()
                }),
                command: "354".into(),
                args: vec![
                    "museun",
                    "152",
                    "#test",
                    "~museun",
                    "test.localhost",
                    "test.localhost",
                    "museun",
                    "H@",
                    "0",
                ].iter()
                    .map(|s| s.to_string())
                    .collect::<Vec<_>>(),
                data: "realname".into(),
            },
        );

        let input = "PING :1344275933";
        let msg = Message::parse(&input);
        assert_eq!(
            msg,
            Message {
                prefix: None,
                command: "PING".into(),
                args: vec![],
                data: "1344275933".into(),
            },
        );

        let input = ":test.localhost 329 museun #test 1532222059";
        let msg = Message::parse(&input);
        assert_eq!(
            msg,
            Message {
                prefix: Some(Prefix::Server {
                    host: "test.localhost".into()
                }),
                command: "329".into(),
                args: vec!["museun", "#test", "1532222059"]
                    .iter()
                    .map(|s| s.to_string())
                    .collect::<Vec<_>>(),
                data: "".into(),
            },
        );
    }
}
*/
