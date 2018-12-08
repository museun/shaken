#![allow(dead_code)]
use curl::easy::{Easy, List};
use log::*;
use serde_derive::Deserialize;

pub struct TwitchClient {
    client_id: String,
}

impl Default for TwitchClient {
    fn default() -> Self {
        Self::new()
    }
}

impl TwitchClient {
    pub fn new() -> Self {
        let client_id = match std::env::var("SHAKEN_TWITCH_CLIENT_ID") {
            Ok(client_id) => client_id,
            Err(_err) => {
                error!("env variable must be set: SHAKEN_TWITCH_CLIENT_ID");
                std::process::exit(1);
            }
        };

        Self { client_id }
    }

    pub fn get_streams<'a>(&self, user_logins: impl AsRef<[&'a str]>) -> Option<Vec<Stream>> {
        let mut map = Vec::new();
        for &login in user_logins.as_ref() {
            map.push(("user_login", login));
        }

        self.get_response("streams", &map)
    }

    pub fn get_streams_from_ids<'a>(&self, ids: impl AsRef<[&'a str]>) -> Option<Vec<Stream>> {
        let mut map = Vec::new();
        for &id in ids.as_ref() {
            map.push(("user_id	", id));
        }

        self.get_response("streams", &map)
    }

    pub fn get_users<'a>(&self, user_logins: impl AsRef<[&'a str]>) -> Option<Vec<User>> {
        let mut map = Vec::new();
        for &login in user_logins.as_ref() {
            map.push(("login", login));
        }

        self.get_response("users", &map)
    }

    pub fn get_users_from_ids<'a>(&self, ids: impl AsRef<[&'a str]>) -> Option<Vec<User>> {
        let mut map = Vec::new();
        for &id in ids.as_ref() {
            map.push(("id", id));
        }

        self.get_response("users", &map)
    }

    pub(crate) fn get_response<'a, T>(
        &self,
        ep: &str,
        map: impl AsRef<[(&'a str, &'a str)]>,
    ) -> Option<Vec<T>>
    where
        for<'de> T: serde::Deserialize<'de>,
    {
        const BASE_URL: &str = "https://api.twitch.tv/helix";

        let mut query = String::from("?");
        for (k, v) in map.as_ref() {
            query.push_str(&format!("{}={}&", encode(k), encode(v)));
        }

        let mut vec = Vec::new();
        let mut easy = Easy::new();

        let mut list = List::new();
        list.append(&format!("Client-ID: {}", self.client_id))
            .unwrap();
        easy.http_headers(list).unwrap();

        let url = format!("{}/{}{}", BASE_URL, ep, query);
        trace!("getting: {}", &url);

        easy.url(&url).ok()?;
        {
            let mut transfer = easy.transfer();
            let _ = transfer.write_function(|data| {
                vec.extend_from_slice(data);
                Ok(data.len())
            });
            transfer
                .perform()
                .map_err(|e| error!("cannot perform transfer: {}", e))
                .ok()?;
        }

        let value = serde_json::from_slice::<serde_json::Value>(&vec)
            .map_err(|err| {
                error!("parse json: {}", err);
                err
            })
            .ok()?;

        let value = value
            .get("data")
            .or_else(|| {
                error!("cannot get 'data' from json value");
                None
            })?
            .clone(); // why is this being cloned?

        serde_json::from_value(value)
            .map_err(|e| {
                error!("cannot convert : {}", e);
                e
            })
            .ok()
    }

    pub fn get_names_for<S: AsRef<str>>(ch: S) -> Option<Names> {
        let url = format!("https://tmi.twitch.tv/group/user/{}/chatters", ch.as_ref());
        if let Some(resp) = crate::util::http_get(&url) {
            return serde_json::from_str::<Names>(&resp)
                .map_err(|e| error!("cannot parse json: {}", e))
                .ok();
        }
        None
    }
}

fn encode(data: &str) -> String {
    let mut res = String::new();
    for ch in data.as_bytes().iter() {
        match *ch as char {
            'A'..='Z' | 'a'..='z' | '0'..='9' | '-' | '_' | '.' | '~' => res.push(*ch as char),
            ch => res.push_str(format!("%{:02X}", ch as u32).as_str()),
        }
    }
    res
}

#[derive(Deserialize, Debug)]
pub struct User {
    pub id: String,
    pub login: String,
    pub display_name: String,
    #[serde(rename = "type")]
    pub role: String,
    pub description: String,
}

#[derive(Deserialize, Debug)]
pub struct Stream {
    pub id: String,
    pub user_id: String,
    pub game_id: String,
    #[serde(rename = "type")]
    pub live: String,
    pub title: String,
    pub viewer_count: usize,
    pub started_at: String, // this should be a timestamp
}

#[derive(Deserialize, Debug)]
pub struct Chatters {
    pub moderators: Vec<String>,
    pub staff: Vec<String>,
    pub admins: Vec<String>,
    pub global_mods: Vec<String>,
    pub viewers: Vec<String>,
}

#[derive(Deserialize, Debug)]
pub struct Names {
    pub chatter_count: usize,
    pub chatters: Chatters,
}
