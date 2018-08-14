#[derive(Clone, Debug, PartialEq)]
pub struct Request<'a> {
    name: &'a str,      // head
    args: Vec<&'a str>, // tail
    sender: i64,
}

impl<'a> Request<'a> {
    pub fn try_parse(sender: i64, data: &'a str) -> Option<Self> {
        if sender == -1 {
            warn!("invalid sender id");
            return None;
        }

        if data.starts_with('!') {
            let mut parts = data.split_whitespace();
            Some(Request {
                name: parts.next()?,
                args: parts.map(|s| s.trim()).collect(),
                sender,
            })
        } else {
            None
        }
    }

    pub fn name(&self) -> &'a str {
        self.name
    }

    pub fn args(&self) -> Vec<&'a str> {
        self.args
    }

    pub fn sender(&self) -> i64 {
        self.sender
    }

    pub fn search(&self, query: &str) -> Option<Request<'a>> {
        if self.name == query {
            return Some(self.clone());
        }

        for (depth, arg) in self.args.iter().enumerate() {
            if *arg == query {
                let req = Request {
                    name: arg,
                    args: self.args.iter().skip(depth + 1).map(|s| *s).collect(),
                    sender: self.sender,
                };
                return Some(req);
            }
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
        let req = Request::try_parse(0, input);
        assert_eq!(
            req,
            Some(Request {
                sender: 0,
                name: "this",
                args: vec!["is", "a", "test"]
            })
        );

        let input = "!hello";
        let req = Request::try_parse(0, input);
        assert_eq!(
            req,
            Some(Request {
                sender: 0,
                name: "hello",
                args: vec![]
            })
        );

        let input = "!";
        let req = Request::try_parse(0, input);
        assert_eq!(req, None);

        let input = "foo bar";
        let req = Request::try_parse(0, input);
        assert_eq!(req, None);
    }

    #[test]
    fn search_request() {
        let input = "!this";
        let req = Request::try_parse(0, input);
        assert_eq!(
            req,
            Some(Request {
                sender: 0,
                name: "this",
                args: vec![]
            })
        );

        let req = req.unwrap().search("this");
        assert_eq!(
            req,
            Some(Request {
                sender: 0,
                name: "this",
                args: vec![]
            })
        );

        let req = req.unwrap().search("test");
        assert_eq!(req, None);
    }

    #[test]
    fn search_sub_request() {
        let input = "!this is a test";
        let req = Request::try_parse(0, input);
        assert_eq!(
            req,
            Some(Request {
                sender: 0,
                name: "this",
                args: vec!["is", "a", "test"]
            })
        );

        let req = req.unwrap().search("is");
        assert_eq!(
            req,
            Some(Request {
                sender: 0,
                name: "is",
                args: vec!["a", "test"]
            })
        );

        let req = req.unwrap().search("this");
        assert_eq!(req, None);

        let input = "!this is a test";
        let req = Request::try_parse(0, input);
        let req = req.unwrap().search("test");
        assert_eq!(
            req,
            Some(Request {
                sender: 0,
                name: "test",
                args: vec![]
            })
        );
    }
}
