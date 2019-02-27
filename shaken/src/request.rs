use crate::prelude::*;

#[derive(Clone, Debug, PartialEq, Default)]
pub struct Request {
    name: Option<String>, // head starts out empty
    args: String,         // tail
    sender: i64,
    target: String,
    broadcaster: bool,
    moderator: bool,
    color: RGB,
}

// TODO: I don't like this. rework this whole thing
// (args and args_iter being totally different is .. awkward)

impl Request {
    pub fn try_from(msg: &irc::Message) -> Option<Self> {
        match (msg.command.as_str(), msg.data.as_ref()) {
            ("PRIVMSG", Some(data)) | ("WHISPER", Some(data))
                if data.starts_with('!') && data.len() > 1 =>
            {
                let sender = User::from_msg(&msg)?;
                let (broadcaster, moderator) = (
                    msg.tags.has_badge(irc::Badge::Broadcaster),
                    msg.tags.has_badge(irc::Badge::Moderator),
                );

                Some(Request {
                    name: None,
                    args: data.to_string(),
                    sender,
                    target: msg.target().to_string(),
                    broadcaster,
                    moderator,
                    color: msg.tags.get_color(),
                })
            }
            _ => None,
        }
    }

    pub fn name(&self) -> Option<&String> {
        self.name.as_ref()
    }

    pub fn args(&self) -> &str {
        &self.args
    }

    pub fn args_iter(&self) -> impl Iterator<Item = &str> {
        self.args.split_whitespace().map(str::trim)
    }

    pub fn target(&self) -> &str {
        &self.target
    }

    pub fn sender(&self) -> i64 {
        self.sender
    }

    pub fn is_from_owner(&self) -> bool {
        Config::load().twitch.owners.contains(&self.sender)
    }

    pub fn color(&self) -> RGB {
        self.color
    }

    pub fn is_from_moderator(&self) -> bool {
        self.moderator
    }

    pub fn is_from_broadcaster(&self) -> bool {
        self.broadcaster
    }

    pub fn search(&self, query: &str) -> Option<Request> {
        if query == "!" {
            return None;
        }

        if let Some(name) = &self.name {
            if *name == query {
                return Some(self.clone());
            }
        }

        if self.args.starts_with(&query) {
            return Some(Request {
                name: Some(query.to_string()),
                args: self.args[query.len()..].trim().to_string(),
                sender: self.sender,
                target: self.target.clone(),
                moderator: self.moderator,
                broadcaster: self.broadcaster,
                color: self.color,
            });
        }

        None
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn search_request() {
        let req = Request {
            name: None,
            args: "!this".into(),
            target: "#foo".into(),
            ..Request::default()
        };
        let req = req.search("!this");
        assert_eq!(
            req,
            Some(Request {
                name: Some("!this".into()),
                args: "".into(),
                target: "#foo".into(),
                ..Request::default()
            })
        );
        let req = req.unwrap().search("test");
        assert_eq!(req, None);

        let req = Request {
            name: None,
            args: "!this test".into(),
            target: "#foo".into(),
            ..Request::default()
        };
        let req = req.search("!this");
        assert_eq!(
            req,
            Some(Request {
                name: Some("!this".into()),
                args: "test".into(),
                target: "#foo".into(),
                ..Request::default()
            })
        );

        let req = req.unwrap().search("test");
        assert_eq!(
            req,
            Some(Request {
                name: Some("test".into()),
                args: "".into(),
                target: "#foo".into(),
                ..Request::default()
            })
        );

        let req = req.unwrap().search("bar");
        assert_eq!(req, None);
    }

    #[test]
    fn search_sub_request() {
        let req = Request {
            name: None,
            args: "!this is a test".into(),
            target: "#test".into(),
            ..Request::default()
        };

        let req = req.search("!this is");
        assert_eq!(
            req,
            Some(Request {
                name: Some("!this is".into()),
                args: "a test".into(),
                target: "#test".into(),
                ..Request::default()
            })
        );

        let req = req.unwrap().search("this");
        assert_eq!(req, None);

        let req = Request {
            name: None,
            args: "!this is a test".into(),
            target: "#test".into(),
            ..Request::default()
        };
        let req = req.search("test");
        assert_eq!(req, None);
    }

}
