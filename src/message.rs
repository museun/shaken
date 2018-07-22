#![allow(dead_code, unused_variables)] // go away

use std::fmt;

#[derive(Debug)]
pub enum MessageError {}

impl fmt::Display for MessageError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        Ok(())
    }
}

#[derive(Debug, PartialEq)]
pub struct Message<'a> {
    pub prefix: Option<Prefix<'a>>,
    pub command: &'a str,
    pub args: Vec<&'a str>,
    pub data: Option<&'a str>,
}

impl<'a> Message<'a> {
    // should probably return a result
    pub fn parse(input: &'a str) -> Message<'a> {
        let prefix = Prefix::parse(&input);

        let iter = input
            .split_whitespace()
            .skip(if prefix.is_some() { 1 } else { 0 });

        let mut args = iter.take_while(|s| !s.starts_with(':')).collect::<Vec<_>>();
        let command = args.remove(0);

        let data = if let Some(pos) = &input[1..].find(':') {
            let data = &input[*pos + 2..];
            Some(data)
        } else {
            None
        };

        Self {
            prefix,
            command,
            args,
            data,
        }
    }
}

#[derive(Debug, PartialEq)]
pub enum Prefix<'a> {
    User {
        nick: &'a str,
        user: &'a str,
        host: &'a str,
    },
    Server {
        host: &'a str,
    },
}

impl<'a> Prefix<'a> {
    pub fn parse(input: &'a str) -> Option<Self> {
        if !input.starts_with(':') {
            None?;
        }

        let s = &input[1..input.find(' ')?];
        match s.find('!') {
            Some(pos) => {
                let nick = &s[..pos];
                let at = s.find('@')?;
                let user = &s[pos + 1..at];
                let host = &s[at + 1..];
                Some(Prefix::User { nick, user, host })
            }
            None => Some(Prefix::Server { host: s }),
        }
    }
}

impl<'a> fmt::Display for Prefix<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Prefix::User { nick, user, host } => writeln!(f, "{}!{}@{}", nick, user, host),
            Prefix::Server { host } => writeln!(f, "{}", host),
        }
    }
}

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
                nick: "test",
                user: "some",
                host: "user.localhost"
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
                host: "irc.localhost"
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
                    host: "test.localhost"
                }),
                command: "001",
                args: vec!["museun"],
                data: Some("Welcome to the Internet Relay Network museun"),
            },
        );

        let input = ":museun!~museun@test.localhost JOIN :#test";
        let msg = Message::parse(&input);
        assert_eq!(
            msg,
            Message {
                prefix: Some(Prefix::User {
                    nick: "museun",
                    user: "~museun",
                    host: "test.localhost"
                }),
                command: "JOIN",
                args: vec![],
                data: Some("#test"),
            },
        );

        let input = ":test.localhost 354 museun 152 #test ~museun test.localhost test.localhost museun H@ 0 :realname";
        let msg = Message::parse(&input);
        assert_eq!(
            msg,
            Message {
                prefix: Some(Prefix::Server {
                    host: "test.localhost"
                }),
                command: "354",
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
                ],
                data: Some("realname"),
            },
        );

        let input = "PING :1344275933";
        let msg = Message::parse(&input);
        assert_eq!(
            msg,
            Message {
                prefix: None,
                command: "PING",
                args: vec![],
                data: Some("1344275933"),
            },
        );

        let input = ":test.localhost 329 museun #test 1532222059";
        let msg = Message::parse(&input);
        assert_eq!(
            msg,
            Message {
                prefix: Some(Prefix::Server {
                    host: "test.localhost"
                }),
                command: "329",
                args: vec!["museun", "#test", "1532222059"],
                data: None,
            },
        );
    }
}
