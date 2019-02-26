#[macro_export]
macro_rules! bail {
    ($e:expr) => {
        match $e {
            Some(item) => item,
            None => return,
        }
    };
}

#[macro_export]
macro_rules! multi {
    ($($arg:expr),* $(,)*) => {{
        use crate::prelude::Response;
        let mut vec = vec![];

        $(
            if let Some(arg) = $arg {
                vec.push(arg);
            }
        )*

        Some(Response::Multi{data: vec})
    }};
}

#[macro_export]
macro_rules! reply {
    ($($arg:tt)*) => {{
        use crate::prelude::Response;
        Some(Response::Reply{data: format!($($arg)*)})
    }};
}

#[macro_export]
macro_rules! say {
    ($($arg:tt)*) => {{
        use crate::prelude::Response;
        Some(Response::Say{data: format!($($arg)*)})
    }}
}

#[macro_export]
macro_rules! action {
    ($($arg:tt)*) => {{
        use crate::prelude::Response;
        Some(Response::Action{data: format!($($arg)*)})
    }};
}

#[macro_export]
macro_rules! whisper {
    ($($arg:tt)*) => {{
        use crate::prelude::Response;
        Some(Response::Whisper{data: format!($($arg)*)})
    }};
}

#[macro_export]
macro_rules! raw {
    ($($arg:tt)*) => {{
        use crate::prelude::{Response, IrcCommand};
       Some(Response::Command{cmd: IrcCommand::Raw{ data: format!($($arg)*) }})
    }};
}

#[macro_export]
macro_rules! privmsg {
    ($target:expr, $($arg:tt)*) => {{
        use crate::prelude::{Response, IrcCommand};
        Some(Response::Command {
            cmd: IrcCommand::Privmsg{
                target: $target.to_string(),
                data: format!($($arg)*)
            }
        })
    }};
}

#[macro_export]
macro_rules! require_owner {
    ($req:expr) => {{
        if !$req.is_from_owner() {
            return None;
        };
        $req
    }};
    ($req:expr, $reason:expr) => {{
        if !$req.is_from_owner() {
            return reply!($reason);
        };
        $req
    }};
}

#[macro_export]
macro_rules! require_broadcaster {
    ($req:expr) => {{
        if !$req.is_from_broadcaster() {
            return None;
        };
        $req
    }};
    ($req:expr, $reason:expr) => {{
        if !$req.is_from_broadcaster() {
            return reply!($reason);
        };
        $req
    }};
}

#[macro_export]
macro_rules! require_moderator {
    ($req:expr) => {{
        if !$req.is_from_moderator() {
            return None;
        };
        $req
    }};
    ($req:expr, $reason:expr) => {{
        if !$req.is_from_moderator() {
            return reply!($reason);
        };
        $req
    }};
}

#[macro_export]
macro_rules! require_privileges {
    ($req:expr) => {{
        if !$req.is_from_owner() && !$req.is_from_broadcaster() && !$req.is_from_moderator() {
            return None;
        };
        $req
    }};
    ($req:expr, $reason:expr) => {{
        if !$req.is_from_owner() && !$req.is_from_broadcaster() && !$req.is_from_moderator() {
            return reply!($reason);
        };
        $req
    }};
}

// unused but not forgotten
#[macro_export]
macro_rules! map {
    (@one $($x:tt)*) => (());

    (@len $($e:expr),*) => (<[()]>::len(&[$(map!(@one $e)),*]));

    ($($k:expr => $v:expr),*) => {{
        let mut _map = hashbrown::HashMap::with_capacity(map!(@len $($k),*));
        $( let _ = _map.insert($k.to_string(), $v.to_string()); )*
        _map
    }};
}

use hashbrown::HashMap;
use log::*;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

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
    pub fn get<K>(&self, k: &K) -> Option<&str>
    where
        K: ?Sized + std::hash::Hash + Eq,
        String: std::borrow::Borrow<K>,
    {
        self.map.get(k).map(String::as_str)
    }

    pub fn get_mut<K>(&mut self, k: &K) -> Option<&mut str>
    where
        K: ?Sized + std::hash::Hash + Eq,
        String: std::borrow::Borrow<K>,
    {
        self.map.get_mut(k).map(String::as_mut_str)
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
    fn parse_resp() {
        let rf = ResponseFinder::load();
        eprintln!("{:#?}", rf);
    }
}
