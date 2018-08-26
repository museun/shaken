use super::{super::*, Prefix};
use std::fmt;

// TODO get rid of all of these string allocations
#[derive(Debug, PartialEq, Clone, Default)]
pub struct Message {
    pub tags: Tags,
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
            (input, Tags::default())
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

    // TODO: make sure it has caps before sending to this function
    fn parse_tags(input: &str) -> (&str, Tags) {
        let pos = input.find(' ').unwrap();
        let sub = &input[..pos];
        let tags = Tags::new(&sub);
        (&input[pos + 1..], tags)
    }

    /// this panics if the message state is bad
    pub fn target(&self) -> &str {
        let target = self.args.first().expect("should have a target");

        let user = UserStore::get_bot(
            &crate::database::get_connection(),
            &Config::load().twitch.name,
        ).expect("to get our name");

        if target.eq_ignore_ascii_case(&user.display) {
            match self.prefix {
                Some(Prefix::User { ref nick, .. }) => &nick,
                _ => unreachable!(),
            }
        } else {
            &target
        }
    }

    pub fn command(&self) -> &str {
        &self.command
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
