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
            return None;
        }

        let s = input[1..input.find(' ')?].trim();
        match s.find('!') {
            Some(pos) => {
                let at = s.find('@')?;
                Some(Prefix::User {
                    nick: s[..pos].into(),
                    user: s[pos + 1..at].into(),
                    host: s[at + 1..].into(),
                })
            }
            None => Some(Prefix::Server { host: s.into() }),
        }
    }
}
