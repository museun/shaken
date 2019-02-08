use crate::prelude::*;

#[derive(Debug, PartialEq, Clone, Default)]
pub struct Message {
    pub tags: irc::Tags,
    pub prefix: Option<irc::Prefix>,
    pub command: String,
    pub args: Vec<String>,
    pub data: Option<String>,
}

impl Message {
    pub fn parse(input: &str) -> Message {
        let (input, tags) =
            if !input.starts_with(':') && !input.starts_with("PING") && input.starts_with('@') {
                let pos = input.find(' ').unwrap();
                let sub = &input[..pos];
                let tags = irc::Tags::new(&sub);
                (&input[pos + 1..], tags)
            } else {
                (input, irc::Tags::default())
            };

        let prefix = irc::Prefix::parse(&input);

        let skip = if prefix.is_some() { 1 } else { 0 };
        let mut args = input
            .split_whitespace()
            .skip(skip)
            .take_while(|s| !s.starts_with(':'))
            .map(|s| s.into());

        let command = args.next().unwrap();
        let data = if let Some(pos) = &input[1..].find(':') {
            Some(input[*pos + 2..].into())
        } else {
            None
        };

        Self {
            tags,
            prefix,
            command,
            args: args.collect(),
            data,
        }
    }

    pub fn target(&self) -> &str {
        let target = self.args.first().expect("should have a target");
        let user = UserStore::get_bot(&get_connection()).expect("get our name");
        if target.eq_ignore_ascii_case(&user.display) {
            match self.prefix {
                Some(irc::Prefix::User { ref nick, .. }) => &nick,
                _ => unreachable!(),
            }
        } else {
            &target
        }
    }

    pub fn expect_data(&self) -> &str {
        self.data.as_ref().unwrap()
    }

    pub fn command(&self) -> &str {
        &self.command
    }
}
