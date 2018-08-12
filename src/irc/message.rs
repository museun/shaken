use std::collections::HashMap;
use std::fmt;

// TODO get rid of all of these string allocations
#[derive(Debug, PartialEq, Clone, Default)]
pub struct Message {
    pub tags: HashMap<String, String>,
    pub prefix: Option<Prefix>,
    pub command: String,
    pub args: Vec<String>,
    pub data: String,
}

impl Message {
    // TODO: should probably return a result
    pub fn parse(input: &str) -> Message {
        let (input, tags) = if !input.starts_with(':') && !input.starts_with("PING") {
            Self::parse_tags(&input)
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

    // make sure it has caps before sending to this function
    fn parse_tags(input: &str) -> (&str, HashMap<String, String>) {
        let mut map = HashMap::new();
        let pos = input.find(' ').unwrap();
        let sub = &input[..pos];
        for part in sub.split_terminator(';') {
            if let Some(index) = part.find('=') {
                let (k, v) = (&part[..index], &part[index + 1..]);
                map.insert(k.into(), v.into());
            }
        }
        (&input[pos + 1..], map)
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
