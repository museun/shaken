#![allow(dead_code)]
use curl::easy::{Easy, List};

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
        Self {
            client_id: env!("SHAKEN_TWITCH_CLIENT_ID").to_string(),
        }
    }

    pub fn get_streams<V, S>(&self, user_logins: V) -> Option<Vec<Stream>>
    where
        S: AsRef<str>,
        V: AsRef<[S]>,
    {
        let mut map = Vec::new();
        for login in user_logins.as_ref() {
            map.push(("user_login", login.as_ref()));
        }

        self.get_response("streams", &map)
    }

    pub fn get_streams_from_ids<V, S>(&self, ids: V) -> Option<Vec<Stream>>
    where
        S: AsRef<str>,
        V: AsRef<[S]>,
    {
        let mut map = Vec::new();
        for id in ids.as_ref() {
            map.push(("user_id	", id.as_ref()));
        }

        self.get_response("streams", &map)
    }

    pub fn get_users<V, S>(&self, user_logins: V) -> Option<Vec<User>>
    where
        S: AsRef<str>,
        V: AsRef<[S]>,
    {
        let mut map = Vec::new();
        for login in user_logins.as_ref() {
            map.push(("login", login.as_ref()));
        }

        self.get_response("users", &map)
    }

    pub fn get_users_from_ids<V, S>(&self, ids: V) -> Option<Vec<User>>
    where
        S: AsRef<str>,
        V: AsRef<[S]>,
    {
        let mut map = Vec::new();
        for id in ids.as_ref() {
            map.push(("id", id.as_ref()));
        }

        self.get_response("users", &map)
    }

    pub(crate) fn get_response<T, S, V>(&self, ep: &str, map: V) -> Option<Vec<T>>
    where
        for<'de> T: serde::Deserialize<'de>,
        S: AsRef<str>,
        V: AsRef<[(S, S)]>,
    {
        const BASE_URL: &str = "https://api.twitch.tv/helix";

        let mut query = String::from("?");
        for (k, v) in map.as_ref() {
            query.push_str(&format!("{}={}&", encode(k.as_ref()), encode(v.as_ref()),));
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
