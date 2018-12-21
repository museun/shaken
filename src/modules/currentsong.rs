use crate::prelude::*;
use std::collections::VecDeque;
use std::net::TcpListener;
use std::sync::{Arc, RwLock};

use log::*;
use serde_derive::{Deserialize, Serialize};

pub struct CurrentSong {
    map: CommandMap<CurrentSong>,
    listener: Listener,
    pattern: regex::Regex,
}

impl Module for CurrentSong {
    fn command(&mut self, req: &Request) -> Option<Response> {
        let map = self.map.shallow_clone();
        map.dispatch(self, req) // why isn't this automatically implemented?
    }
}

impl CurrentSong {
    pub fn create() -> Result<Self, ModuleError> {
        let pattern = regex::Regex::new(
               r#"(:?(:?^(:?http?.*?youtu(:?\.be|be.com))(:?/|.*?v=))(?P<id>[A-Za-z0-9_-]{11}))|(?P<id2>^[A-Za-z0-9_-]{11}$)"#,
            ).unwrap();

        Ok(Self {
            map: CommandMap::create(
                "CurrentSong",
                &[
                    ("!song", Self::info_command), //
                ],
            )?,
            // TODO get this from the config
            listener: Listener::start().map_err(|err| {
                error!("cannot bind to localhost:50005: {}", err);
                ModuleError::CannotStart
            })?,
            pattern,
        })
    }

    fn info_command(&mut self, _: &Request) -> Option<Response> {
        if let Some(info) = self.listener.current() {
            let id = self
                .pattern
                .captures(&info.url)
                .and_then(|s| s.name("id"))?
                .as_str();

            if let Some(vid) = lookup_info(id) {
                return multi!(
                    say!("{}", vid.title), //
                    say!("{}", info.url)   //
                );
            } else {
                return say!("{}", info.url);
            }
        }

        reply!("no song is currently playing")
    }
}

#[derive(Debug, Clone, PartialEq, PartialOrd, Deserialize, Serialize)]
struct Info {
    ts: u64,
    url: String,
    title: String,
}

struct Listener {
    list: Arc<RwLock<VecDeque<Info>>>,
}

impl Listener {
    pub fn start() -> Result<Self, std::io::Error> {
        let listener = TcpListener::bind("localhost:50005")?;
        let list = Arc::new(RwLock::new(VecDeque::new()));

        {
            let list = Arc::clone(&list);
            std::thread::spawn(move || {
                for client in listener.incoming() {
                    if let Some(resp) = client
                        .map_err(|err| error!("cannot accept client: {}", err))
                        .ok()
                        .and_then(|mut client| {
                            serde_json::from_reader(&mut client)
                                .map_err(|err| error!("cannot deserialize json: {}", err))
                                .ok()
                        })
                    {
                        list.write().unwrap().push_back(resp)
                    }
                }
            });
        }

        Ok(Self { list })
    }

    pub fn current(&self) -> Option<Info> {
        self.list.read().unwrap().back().cloned() // to not block
    }

    pub fn list(&self) -> Vec<Info> {
        let mut list = vec![];
        for el in self.list.read().unwrap().iter() {
            list.push(el.clone())
        }
        list
    }
}

struct YoutubeVideo {
    id: String,
    title: String,
}

fn lookup_info(id: &str) -> Option<YoutubeVideo> {
    const BASE: &str = "https://www.googleapis.com/youtube/v3";

    let api_key = std::env::var("SHAKEN_YOUTUBE_API_KEY")
        .map_err(|_| error!("SHAKEN_YOUTUBE_API_KEY is not set. not getting title"))
        .ok()?;

    let map = &[
        ("id", id),
        ("part", "snippet"),
        ("fields", "items(id, snippet(title))"),
        ("key", api_key.as_str()),
    ];

    let query = std::iter::once("?".into()) // TODO use a fold here
        .chain(
            map.as_ref()
                .iter()
                .map(|(k, v)| format!("{}={}&", util::encode(k), util::encode(v))),
        )
        .collect::<String>();

    let mut response = vec![];

    use curl::easy::Easy;
    let mut easy = Easy::new();
    easy.url(&format!("{}/videos/{}", BASE, query)).ok()?;
    {
        let mut transfer = easy.transfer();
        transfer
            .write_function(|data| {
                response.extend_from_slice(&data);
                Ok(data.len())
            })
            .ok()?;
        transfer
            .perform()
            .map_err(|e| error!("cannot perform transfer: {}", e))
            .ok()?;
    }

    let val = serde_json::from_slice::<serde_json::Value>(&response)
        .map_err(|e| error!("cannot perform transfer: {}", e))
        .ok()?;

    let val = val.get("items")?.as_array()?.get(0)?;
    let id = val.get("id")?.as_str()?;
    let title = val
        .get("snippet")
        .and_then(|val| val.get("title"))?
        .as_str()?;

    Some(YoutubeVideo {
        id: id.to_string(),
        title: title.to_string(),
    })
}
