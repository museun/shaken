use crate::prelude::*;

#[derive(Clone, Debug, PartialEq, Default)]
pub struct Request<'a> {
    name: Option<&'a str>, // head starts out empty
    args: &'a str,         // tail
    sender: i64,
    target: &'a str,
}

impl<'a> Request<'a> {
    pub fn try_parse(target: &'a str, sender: i64, data: &'a str) -> Option<Self> {
        if sender == -1 {
            warn!("invalid sender id");
            return None;
        }

        // TODO make this prefix configurable
        if data.starts_with('!') && data.len() > 1 {
            return Some(Request {
                name: None,
                args: data,
                sender,
                target,
            });
        }

        None
    }

    pub fn name(&self) -> Option<&'a str> {
        self.name
    }

    pub fn args(&self) -> &'a str {
        self.args
    }

    pub fn args_iter(&self) -> impl Iterator<Item = &'a str> {
        self.args.split_whitespace().map(|s| s.trim())
    }

    pub fn target(&self) -> &'a str {
        self.target
    }

    pub fn sender(&self) -> i64 {
        self.sender
    }

    pub fn is_from_owner(&self) -> bool {
        Config::load()
            .twitch
            .owners
            .iter()
            .any(|&id| id == self.sender)
    }

    pub fn search(&self, query: &'a str) -> Option<Request<'a>> {
        // TODO make this configurable (a prefix)
        if query == "!" {
            return None;
        }

        if let Some(name) = self.name {
            if name == query {
                return Some(self.clone());
            }
        }

        if self.args.starts_with(query) {
            return Some(Request {
                name: Some(&query), // this needs to live for as long as the new req
                args: &self.args[query.len()..].trim(),
                sender: self.sender,
                target: self.target,
            });
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn make_request() {
        let input = "!this is a test";
        let _req = Request::try_parse("#test", 0, input).unwrap();

        let input = "!hello";
        let _req = Request::try_parse("#test", 0, input).unwrap();

        let input = "!";
        assert_eq!(Request::try_parse("#test", 0, input), None);

        let input = "foo bar";
        assert_eq!(Request::try_parse("#test", 0, input), None);
    }

    #[test]
    fn search_request() {
        let input = "!this";
        let req = Request::try_parse("#test", 0, input).unwrap();

        let req = req.search("!this");
        assert_eq!(
            req,
            Some(Request {
                sender: 0,
                name: Some("!this"),
                args: "",
                target: "#test",
            })
        );

        let req = req.unwrap().search("test");
        assert_eq!(req, None);
    }

    #[test]
    fn search_sub_request() {
        let input = "!this is a test";
        let req = Request::try_parse("#test", 0, input).unwrap();
        let req = req.search("!this is");
        assert_eq!(
            req,
            Some(Request {
                sender: 0,
                name: Some("!this is"),
                args: "a test",
                target: "#test",
            })
        );

        let req = req.unwrap().search("this");
        assert_eq!(req, None);

        let input = "!this is a test";
        let req = Request::try_parse("#test", 0, input).unwrap();
        let req = req.search("test");
        assert_eq!(req, None);
    }

    #[test]
    fn is_from_owner() {
        let req = Request::try_parse("#test", 23196011, "!this is a test").unwrap();
        assert_eq!(req.is_from_owner(), true);

        let req = Request::try_parse("#test", 42, "!this is a test").unwrap();
        assert_eq!(req.is_from_owner(), false);
    }
}
