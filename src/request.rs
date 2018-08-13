#[derive(Clone, Debug, PartialEq)]
pub struct Request<'a> {
    name: &'a str,      // head
    args: Vec<&'a str>, // tail
}

impl<'a> Request<'a> {
    pub fn try_parse(data: &'a str) -> Option<Self> {
        if data.starts_with('!') {
            let mut parts = data[1..].split_whitespace();
            Some(Request {
                name: parts.next()?,
                args: parts.map(|s| s.trim()).collect(),
            })
        } else {
            None
        }
    }

    /// Searches for the first instance of the subcommand, returning it as the name, with the rest as the args
    pub fn search(&self, cmd: &str) -> Option<Request<'a>> {
        if self.name == cmd {
            return Some(self.clone());
        }

        for (depth, arg) in self.args.iter().enumerate() {
            if *arg == cmd {
                let req = Request {
                    name: arg,
                    args: self.args.iter().skip(depth + 1).map(|s| *s).collect(),
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
        let req = Request::try_parse(input);
        assert_eq!(
            req,
            Some(Request {
                name: "this",
                args: vec!["is", "a", "test"]
            })
        );

        let input = "!hello";
        let req = Request::try_parse(input);
        assert_eq!(
            req,
            Some(Request {
                name: "hello",
                args: vec![]
            })
        );

        let input = "!";
        let req = Request::try_parse(input);
        assert_eq!(req, None);

        let input = "foo bar";
        let req = Request::try_parse(input);
        assert_eq!(req, None);
    }

    #[test]
    fn search_request() {
        let input = "!this";
        let req = Request::try_parse(input);
        assert_eq!(
            req,
            Some(Request {
                name: "this",
                args: vec![]
            })
        );

        let req = req.unwrap().search("this");
        assert_eq!(
            req,
            Some(Request {
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
        let req = Request::try_parse(input);
        assert_eq!(
            req,
            Some(Request {
                name: "this",
                args: vec!["is", "a", "test"]
            })
        );

        let req = req.unwrap().search("is");
        assert_eq!(
            req,
            Some(Request {
                name: "is",
                args: vec!["a", "test"]
            })
        );

        let req = req.unwrap().search("this");
        assert_eq!(req, None);

        let input = "!this is a test";
        let req = Request::try_parse(input);
        let req = req.unwrap().search("test");
        assert_eq!(
            req,
            Some(Request {
                name: "test",
                args: vec![]
            })
        );
    }
}
