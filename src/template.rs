use hashbrown::HashMap;
use log::*;
use serde::{Deserialize, Serialize};
use std::ops::Range;
use std::path::PathBuf;

#[derive(Debug)]
pub struct Template<'a> {
    data: &'a str,
    map: Vec<(&'a str, Range<usize>)>,
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
            data: input,
            map: parts
                .into_iter()
                .rev()
                .map(|(start, end)| (&input[start + 2..end - 1], Range { start, end }))
                .collect::<Vec<_>>(),
        })
    }

    pub fn apply(
        mut self,
        parts: &[(&'static str, &dyn ToString)],
    ) -> Result<String, TemplateError<'a>> {
        if parts.len() != self.map.len() {
            return Err(TemplateError::Mismatch(parts.len(), self.map.len()));
        }
        let mut buf = self.data.to_string();
        let mut offset: Option<usize> = None;

        for (key, repr) in parts.iter() {
            let (binding, range) = self.map.pop().unwrap();
            if binding != *key {
                return Err(TemplateError::Unexpected(binding, key));
            }

            let range = match offset {
                Some(off) => Range {
                    start: range.start - off, // why 2
                    end: range.end - off,
                },
                None => range,
            };

            let repr = repr.to_string();
            let (a, b) = (repr.len(), range.end - range.start);
            let diff = if a > b { a - b } else { b - a };

            buf.replace_range(range.clone(), &repr);
            if let Some(off) = offset.as_mut() {
                *off += diff
            } else {
                offset.replace(diff);
            }
        }

        Ok(buf)
    }

    pub fn inner(&self) -> &str {
        self.data
    }
}

#[derive(Debug, PartialEq)]
pub enum TemplateError<'a> {
    Unbalanced(usize),
    Unexpected(&'a str, &'a str),
    Mismatch(usize, usize),
    Missing(&'a str),
}

impl<'a> std::fmt::Display for TemplateError<'a> {
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

impl<'a> std::error::Error for TemplateError<'a> {}

#[derive(Debug, Deserialize, Serialize)]
pub struct ResponseFinder {
    map: HashMap<String, String>,
}

impl Default for ResponseFinder {
    fn default() -> Self {
        let map = include_str!("../data/responses")
            .lines()
            .map(|s| s.split("=>"))
            .filter_map(|mut s| Some((s.next()?, s.next()?)))
            .map(|(k, v)| (k.trim().trim_matches('"'), v.trim().trim_matches('"')))
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();

        ResponseFinder { map }
    }
}

impl ResponseFinder {
    pub fn get<'a, K>(&'a self, k: &'a K) -> Result<Template<'a>, TemplateError>
    where
        K: ?Sized + std::hash::Hash + Eq + AsRef<str>,
        String: std::borrow::Borrow<K>,
    {
        self.map
            .get(k)
            .map(String::as_str)
            .ok_or_else(|| TemplateError::Missing(k.as_ref()))
            .and_then(Template::parse)
    }

    pub fn load() -> Self {
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
        if get_data_file()
            .and_then(|f| std::fs::File::create(f).ok())
            .and_then(|fi| serde_json::to_writer_pretty(fi, &self.map).ok())
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
        assert_eq!(err, TemplateError::Unexpected("test", "a"));

        let template = Template::parse("this has nothing in it").unwrap();
        assert_eq!(template.inner(), "this has nothing in it");
        let s = template.apply(&[]).unwrap();
        assert_eq!(s, "this has nothing in it");
    }
}
