use hashbrown::HashMap;
use log::*;
use serde::{Deserialize, Serialize};
use std::ops::RangeInclusive;
use std::path::PathBuf;

use once_cell::{sync::Lazy, sync_lazy};
use std::sync::RwLock;

static GLOBAL_RESPONSE_FINDER: Lazy<RwLock<ResponseFinder>> = sync_lazy! {
    RwLock::new(ResponseFinder::load())
};

#[derive(Debug, PartialEq)]
pub enum Error {
    EmptyTemplate,
    Unbalanced(usize),
    Unexpected(String, String),
    Mismatch { expected: usize, got: usize },
    Missing(String),
    // EmptyParts,
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::EmptyTemplate => write!(f, "template contains no replacement strings"),
            Error::Unbalanced(start) => write!(f, "unbalanced bracket starting at: {}", start),
            Error::Unexpected(expected, got) => write!(f, "expected {}, got: {}", expected, got),
            Error::Mismatch { expected, got } => {
                write!(f, "mismatch counts expected: {}, got: {}", expected, got)
            }
            Error::Missing(key) => write!(f, "template '{}' is missing", key),
        }
    }
}

impl std::error::Error for Error {}

#[derive(Clone, Debug)]
pub struct Template {
    data: String,                 // total string
    left: String,                 // left most part
    index: RangeInclusive<usize>, // index of left most part
}

impl Template {
    pub fn args<'a>(&self) -> TemplateArgs<'a> {
        TemplateArgs::new()
    }

    pub fn parse(input: &str) -> Result<Self, Error> {
        let mut iter = input.char_indices().peekable();

        let mut start = None;
        while let Some((i, ch)) = iter.next() {
            // TODO: this doesn't balance the brackets.. oops
            if let ('$', Some((_, '{'))) = (ch, iter.peek()) {
                start.replace(i);
            }
            if let ('}', Some(n)) = (ch, start) {
                return Ok(Self {
                    left: input[n + 2..i].into(),
                    index: RangeInclusive::new(n, i),
                    data: input.into(),
                });
            }
        }

        match start {
            Some(n) => Err(Error::Unbalanced(n)),
            None => Err(Error::EmptyTemplate),
        }
    }

    pub fn apply<'repr, I, V>(mut self, parts: I) -> Result<String, Error>
    where
        I: IntoIterator<Item = &'repr (&'repr str, V)> + 'repr,
        I::IntoIter: DoubleEndedIterator,
        V: std::fmt::Display + 'repr,
    {
        let parts = parts
            .into_iter()
            .map(|(k, v)| (k, v.to_string()))
            .collect::<HashMap<_, _>>(); // this order doesn't matter

        debug_assert!(!parts.is_empty());

        let mut seen = 0;
        while seen < parts.len() {
            let part = match parts.get(&self.left.as_str()) {
                Some(part) => part,
                None => return Err(Error::Missing(self.left)),
            };
            self.data.replace_range(self.index.clone(), &part);
            if seen == parts.len() - 1 {
                break;
            }

            let this = match Self::parse(&self.data) {
                Err(Error::EmptyTemplate) => break,
                Err(err) => return Err(err),
                Ok(this) => this,
            };
            std::mem::replace(&mut self, this);
            seen += 1;
        }

        let mut data = self.data.to_string();
        data.shrink_to_fit();
        Ok(data)
    }
}

pub struct TemplateArgs<'a>(HashMap<&'a str, String>);

impl<'a> Default for TemplateArgs<'a> {
    fn default() -> Self {
        Self(HashMap::new())
    }
}

impl<'a> TemplateArgs<'a> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with(mut self, key: &'a str, val: &dyn std::fmt::Display) -> Self {
        self.0.insert(key, val.to_string());
        self
    }

    pub fn build(self) -> Vec<(&'a str, String)> {
        self.0.into_iter().collect()
    }
}

pub struct Response(pub &'static str, pub &'static str);

impl std::fmt::Display for Response {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use heck::SnekCase;
        write!(f, "{} => {}", self.0.to_snek_case(), self.1)
    }
}

#[macro_export]
macro_rules! submit {
    ($($e:expr);* $(;)?) => {
        $( inventory::submit!{ $e } )*
    };
}

#[macro_export]
macro_rules! reply_template {
    ($e:expr) => {{
        reply!(template::finder().get_no_apply($e).unwrap())
    }};

    ($e:expr, $($args:expr),* $(,)?) => {
        reply!(template::lookup($e, &[$($args),*]).unwrap())
    };
}

#[macro_export]
macro_rules! say_template {
    ($e:expr) => {
        say!(template::finder().get_no_apply($e).unwrap())
    };

    ($e:expr, $($args:expr),* $(,)?) => {
        say!(template::lookup($e, &[$($args),*]).unwrap())
    };
}

submit!(
    Response("misc_done", "done");
    Response("misc_invalid_args", "invalid arguments");
    Response("misc_invalid_number", "thats not a number I understand");
    Response("misc_requires_priv", "you cannot do that");
);

inventory::collect!(Response);

pub fn lookup<'repr, V, I>(key: &'repr str, parts: I) -> Result<String, Error>
where
    I: IntoIterator<Item = &'repr (&'repr str, V)> + 'repr,
    I::IntoIter: DoubleEndedIterator,
    V: std::fmt::Display + 'repr,
{
    finder().get(key)?.apply(parts)
}

pub fn finder<'a>() -> std::sync::RwLockReadGuard<'a, ResponseFinder> {
    GLOBAL_RESPONSE_FINDER.read().unwrap()
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ResponseFinder {
    map: HashMap<String, String>,
}

impl Default for ResponseFinder {
    fn default() -> Self {
        let mut map = HashMap::new();
        for el in inventory::iter::<Response> {
            use heck::SnekCase;
            map.insert(el.0.to_snek_case(), el.1.to_string());
        }
        ResponseFinder { map }
    }
}

impl ResponseFinder {
    pub fn get_no_apply(&self, k: impl AsRef<str>) -> Result<String, Error> {
        self.map
            .get(k.as_ref())
            .cloned()
            .ok_or_else(|| Error::Missing(k.as_ref().to_string()))
    }

    pub fn get(&self, k: impl AsRef<str>) -> Result<Template, Error> {
        self.map
            .get(k.as_ref())
            .map(String::as_str)
            .ok_or_else(|| Error::Missing(k.as_ref().to_string()))
            .and_then(Template::parse)
    }

    pub fn load() -> Self {
        if cfg!(test) {
            return Self::default();
        }

        let map: Option<HashMap<String, String>> = get_data_file()
            .and_then(|path| std::fs::File::open(path).ok())
            .and_then(|fi| serde_json::from_reader(fi).ok());

        let mut this = Self::default();
        if let Some(map) = map {
            for (k, v) in map {
                this.map.insert(k, v);
            }
        }
        this
    }

    pub fn save(&self) {
        if cfg!(test) {
            return;
        }

        let map = self
            .map
            .iter()
            .collect::<std::collections::BTreeMap<_, _>>();

        if get_data_file()
            .and_then(|f| std::fs::File::create(f).ok())
            .and_then(|fi| serde_json::to_writer_pretty(fi, &map).ok())
            .is_none()
        {
            error!("cannot save the responses to the json file");
        }
    }
}

impl Drop for ResponseFinder {
    fn drop(&mut self) {
        self.save()
    }
}

fn get_data_file() -> Option<PathBuf> {
    use directories::ProjectDirs;
    ProjectDirs::from("com.github", "museun", "shaken").and_then(|dir| {
        let dir = dir.config_dir();
        std::fs::create_dir_all(&dir)
            .ok()
            .and_then(|_| Some(dir.join("responses.json")))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn basic() {
        let p = Template::parse("${a} ${b}${c}").unwrap();
        let t = p.apply(&[("a", &0), ("b", &1), ("c", &2)]).unwrap();
        assert_eq!(t, "0 12");
    }

    #[test]
    fn apply_iter() {
        let mut base = (b'a'..=b'z')
            .map(|c| format!("${{{}}}", c as char))
            .collect::<Vec<_>>()
            .join(" ");

        for c in b'a'..=b'z' {
            let t = Template::parse(&base).unwrap();
            base = t
                .apply(&[(
                    format!("{}", c as char).as_ref(),
                    format!("{} = {}", c as char, c),
                )])
                .unwrap();
        }

        let expected = "a = 97 b = 98 c = 99 d = 100 e = 101 \
                        f = 102 g = 103 h = 104 i = 105 j = 106 \
                        k = 107 l = 108 m = 109 n = 110 o = 111 \
                        p = 112 q = 113 r = 114 s = 115 t = 116 \
                        u = 117 v = 118 w = 119 x = 120 y = 121 \
                        z = 122";

        assert_eq!(base, expected);
    }

    #[test]
    fn real_template() {
        let template = "you've reached a max of ${max} credits, \
                        out of ${total} total credits with ${success} \
                        successes and ${failure} failures. and I've \
                        'collected' ${overall_total} credits from all of \
                        the failures.";

        let t = Template::parse(&template).unwrap();
        let out = t
            .apply(&[
                ("max", &"218,731"),
                ("total", &"706,917"),
                ("success", &"169"),
                ("failure", &"174"),
                ("overall_total", &"1,629,011"),
            ])
            .unwrap();

        let expected = "you've reached a max of 218,731 credits, \
                        out of 706,917 total credits with 169 \
                        successes and 174 failures. and I've \
                        'collected' 1,629,011 credits from all of \
                        the failures.";
        assert_eq!(out, expected);
    }

    #[test]
    fn with_args() {
        let template = "you've reached a max of ${max} credits, \
                        out of ${total} total credits with ${success} \
                        successes and ${failure} failures. and I've \
                        'collected' ${overall_total} credits from all of \
                        the failures.";

        let t = Template::parse(&template).unwrap();
        eprintln!("{:#?}", t);
        let parts = t
            .args()
            .with("max", &"218,731")
            .with("total", &"706,917")
            .with("success", &"169")
            .with("failure", &"174")
            .with("overall_total", &"1,629,011")
            .build();

        let expected = "you've reached a max of 218,731 credits, \
                        out of 706,917 total credits with 169 \
                        successes and 174 failures. and I've \
                        'collected' 1,629,011 credits from all of \
                        the failures.";

        assert_eq!(t.apply(&parts).unwrap(), expected);
    }
}
