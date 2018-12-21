use crate::prelude::*;
use std::collections::VecDeque;
use std::net::TcpListener;
use std::sync::{Arc, RwLock};
use std::time::Duration;

use chrono::prelude::*;
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
                    ("!song", Self::info_command),     //
                    ("!prevsong", Self::prev_command), //
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
        let now = Utc::now(); // get this as early as possible

        if let Some(info) = self.listener.current() {
            let id = self
                .pattern
                .captures(&info.url)
                .and_then(|s| s.name("id"))?
                .as_str();

            if let Some(vid) = lookup_info(id) {
                let start = Utc.timestamp(info.ts as i64, 0);
                let dur = chrono::Duration::from_std(vid.duration).unwrap();
                let time = dur - (now - start);
                let delta = (dur - time).num_seconds();

                return if delta > 0 {
                    say!("\"{}\" {}&t={}s", vid.title, info.url, delta)
                } else {
                    info!("not currently playing");
                    // TODO maybe note that this isn't actually playing
                    say!("\"{}\" {}", vid.title, info.url)
                };
            } else {
                return say!("{}", info.url);
            }
        }

        reply!("no song is currently playing")
    }

    fn prev_command(&mut self, _: &Request) -> Option<Response> {
        let list = self.listener.list();
        if let Some(info) = list.get(list.len().saturating_sub(2)) {
            let id = self
                .pattern
                .captures(&info.url)
                .and_then(|s| s.name("id"))?
                .as_str();

            let start = Utc.timestamp(info.ts as i64, 0);

            return if let Some(vid) = lookup_info(id) {
                say!(
                    "previous song: (started at {}) \"{}\" {}",
                    start,
                    vid.title,
                    info.url
                )
            } else {
                say!("previous song: (started at {}) {}", start, info.url)
            };
        }

        reply!("I don't remember a song playing then")
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
        self.list.read().unwrap().back().cloned() // to not hold onto the mutex
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
    duration: Duration,
}

fn lookup_info(id: &str) -> Option<YoutubeVideo> {
    const BASE: &str = "https://www.googleapis.com/youtube/v3";

    let api_key = std::env::var("SHAKEN_YOUTUBE_API_KEY")
        .map_err(|_| error!("SHAKEN_YOUTUBE_API_KEY is not set. not getting title"))
        .ok()?;

    let map = &[
        ("id", id),
        ("part", "snippet,contentDetails"),
        (
            "fields",
            "items(id, snippet(title), contentDetails(duration))",
        ),
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

    let duration = val
        .get("contentDetails")
        .and_then(|val| val.get("duration"))?
        .as_str()
        .map(from_iso8601)?;

    Some(YoutubeVideo {
        id: id.to_string(),
        title: title.to_string(),
        duration,
    })
}

fn from_iso8601(period: &str) -> Duration {
    let list = period.split(|c: char| !c.is_numeric());

    let (mut total, mut index) = (0, 0);
    for el in list.rev().filter(|el| !el.is_empty()) {
        total += el.parse::<u64>().unwrap() * u64::pow(60, index);
        index += 1;
    }

    Duration::from_secs(total)
}

// The length of the video. The property value is an ISO 8601 duration. For
// example, for a video that is at least one minute long and less than one hour
// long, the duration is in the format PT#M#S, in which the letters PT indicate
// that the value specifies a period of time, and the letters M and S refer to
// length in minutes and seconds, respectively. The # characters preceding the M
// and S letters are both integers that specify the number of minutes (or
// seconds) of the video. For example, a value of PT15M33S indicates that the
// video is 15 minutes and 33 seconds long.

// If the video is at least one hour long, the duration is in the format
// PT#H#M#S, in which the # preceding the letter H specifies the length of the
// video in hours and all of the other details are the same as described above.
// If the video is at least one day long, the letters P and T are separated, and
// the value's format is P#DT#H#M#S. Please refer to the ISO 8601 specification
// for complete details.
