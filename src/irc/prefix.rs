use std::fmt;

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
            return None;
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
