use hashbrown::HashMap;
use log::*;
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::ops::Range;
use std::path::PathBuf;

#[derive(Debug)]
pub struct Template<'a> {
    data: Cow<'a, str>,
    map: Vec<(Cow<'a, str>, Range<usize>)>,
}

impl<'a> Clone for Template<'a> {
    fn clone(&self) -> Self {
        Self {
            data: self.data.clone(),
            map: self
                .map
                .iter()
                .map(|(c, r)| (c.clone(), r.clone()))
                .collect(),
        }
    }
}

impl<'a> Template<'a> {
    pub fn parse(input: &'a str) -> Result<Self, TemplateError> {
        let mut iter = input.char_indices().peekable();

        let mut parts = vec![];
        let mut start = None;
        while let Some((i, ch)) = iter.next() {
            if start.is_some() {
                if let '}' = ch {
                    parts.push((start.take().unwrap(), i + 1));
                }
            }
            if let '$' = ch {
                if let Some((_, '{')) = iter.peek() {
                    start = Some(i)
                }
            }
        }

        if start.is_some() {
            return Err(TemplateError::Unbalanced(start.unwrap()));
        }

        Ok(Self {
            data: input.into(),
            map: parts
                .into_iter()
                .rev()
                .map(|(start, end)| (input[start + 2..end - 1].into(), Range { start, end }))
                .collect::<Vec<_>>(),
        })
    }

    pub fn apply(&self, parts: Parts) -> Result<String, TemplateError> {
        if parts.len() != self.map.len() {
            return Err(TemplateError::Mismatch(parts.len(), self.map.len()));
        }

        let mut buf = self.data.to_string();
        let mut offset: Option<usize> = None;

        let mut clone = self.map.clone();
        for (key, repr) in parts.iter() {
            let (binding, range) = clone.pop().unwrap();
            if binding != *key {
                return Err(TemplateError::Unexpected(
                    binding.to_string(),
                    key.to_string(),
                ));
            }

            let range = match offset {
                Some(off) => Range {
                    start: range.start - off,
                    end: range.end - off,
                },
                None => range,
            };

            let repr = repr.to_string();
            buf.replace_range(range.clone(), &repr);

            let (a, b) = (repr.len(), range.end - range.start);
            let diff = if a > b { a - b } else { b - a };

            if let Some(off) = offset.as_mut() {
                *off += diff
            } else {
                offset.replace(diff);
            }
        }

        Ok(buf)
    }

    pub fn inner(&self) -> Cow<'_, str> {
        self.data.clone()
    }
}

#[derive(Debug, PartialEq)]
pub enum TemplateError {
    Unbalanced(usize),
    Unexpected(String, String),
    Mismatch(usize, usize),
    Missing(String),
}

impl std::fmt::Display for TemplateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TemplateError::Unbalanced(start) => {
                write!(f, "unbalanced bracket starting at: {}", start)
            }
            TemplateError::Unexpected(expected, got) => {
                write!(f, "expected {}, got: {}", expected, got)
            }
            TemplateError::Mismatch(parts, map) => {
                write!(f, "mismatch counts expected: {}, got: {}", map, parts)
            }
            TemplateError::Missing(key) => write!(f, "template '{}' is missing", key),
        }
    }
}

impl std::error::Error for TemplateError {}

use once_cell::{sync::Lazy, sync_lazy};
use std::sync::RwLock;

static GLOBAL_RESPONSE_FINDER: Lazy<RwLock<ResponseFinder>> = sync_lazy! {
    RwLock::new(ResponseFinder::load())
};

pub type Parts<'a> = &'a [(&'a str, &'a dyn ToString)];

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
    ($e:expr) => {
        reply!(template::lookup($e, &[]).unwrap())
    };

    ($e:expr, $($args:expr),* $(,)?) => {
        reply!(template::lookup($e, &[$($args),*]).unwrap())
    };
}

#[macro_export]
macro_rules! say_template {
    ($e:expr) => {
        say!(template::lookup($e, &[]).unwrap())
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

pub fn lookup(key: &str, parts: Parts) -> Result<String, TemplateError> {
    let rf = GLOBAL_RESPONSE_FINDER.read().unwrap();
    rf.get(key)?.apply(parts)
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
    pub fn get(&self, k: impl AsRef<str>) -> Result<Template<'_>, TemplateError> {
        self.map
            .get(k.as_ref())
            .map(String::as_str)
            .ok_or_else(|| TemplateError::Missing(k.as_ref().to_string()))
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
    fn parse_template() {
        let template = Template::parse("hello ${world}. a ${test} : ${answer}").unwrap();
        let s = template
            .apply(&[
                ("world", &42),        //
                ("test", &false),      //
                ("answer", &"foobar"), //
            ])
            .unwrap();
        assert_eq!(s, "hello 42. a false : foobar");

        let template = Template::parse("hello ${asdf}${bar}").unwrap();
        let s = template
            .apply(&[
                ("asdf", &(2 + 2)), //
                ("bar", &(1 + 1)),  //
            ])
            .unwrap();
        assert_eq!(s, "hello 42");

        let template =
            Template::parse("${another_long_thing} | some              ${spaces}").unwrap();
        let s = template
            .apply(&[
                ("another_long_thing", &"\"this is a string\""), //
                ("spaces", &"-".repeat(30)),                     //
            ])
            .unwrap();
        assert_eq!(
            s,
            "\"this is a string\" | some              ------------------------------"
        );

        let err = Template::parse("asdf ${something").unwrap_err();
        assert_eq!(err, TemplateError::Unbalanced(5)); // start counting from 0

        let template = Template::parse("${test}").unwrap();
        let err = template.apply(&[("a", &0), ("b", &0)]).unwrap_err();
        assert_eq!(err, TemplateError::Mismatch(2, 1));

        let template = Template::parse("${test}").unwrap();
        let err = template.apply(&[("a", &0)]).unwrap_err();
        assert_eq!(err, TemplateError::Unexpected("test".into(), "a".into()));

        let template = Template::parse("this has nothing in it").unwrap();
        assert_eq!(template.inner(), "this has nothing in it");
        let s = template.apply(&[]).unwrap();
        assert_eq!(s, "this has nothing in it");
    }

    #[test]
    fn parse_responses() {
        let resp = ResponseFinder::load();
        eprintln!("{:#?}", resp.map);
    }
}
