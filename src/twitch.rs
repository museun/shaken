use log::*;
use serde::Deserialize;
use std::fmt;
use std::iter::repeat;

pub struct TwitchClient {
    client_id: String,
}

impl TwitchClient {
    pub fn new(client_id: &str) -> Self {
        Self {
            client_id: client_id.to_string(),
        }
    }

    pub fn get_streams<A, I>(&self, user_logins: I) -> Result<Vec<Stream>, Error>
    where
        I: IntoIterator<Item = A>,
        I::Item: AsRef<str>,
    {
        self.get_response("streams", repeat("user_login").zip(user_logins))
    }

    pub fn get_streams_from_ids<A, I>(&self, ids: I) -> Result<Vec<Stream>, Error>
    where
        I: IntoIterator<Item = A>,
        I::Item: AsRef<str>,
    {
        self.get_response("streams", repeat("user_id ").zip(ids))
    }

    pub fn get_users<A, I>(&self, user_logins: I) -> Result<Vec<User>, Error>
    where
        I: IntoIterator<Item = A>,
        I::Item: AsRef<str>,
    {
        self.get_response("users", repeat("login").zip(user_logins))
    }

    pub fn get_users_from_ids<A, I>(&self, ids: I) -> Result<Vec<User>, Error>
    where
        I: IntoIterator,
        I::Item: AsRef<str>,
    {
        self.get_response("users", repeat("id").zip(ids))
    }

    pub(crate) fn get_response<'a, A, I, T>(&self, ep: &str, map: I) -> Result<Vec<T>, Error>
    where
        for<'de> T: serde::Deserialize<'de>,
        I: IntoIterator<Item = (&'a str, A)>,
        A: AsRef<str>,
    {
        const BASE_URL: &str = "https://api.twitch.tv/helix";
        let mut req = ureq::get(&format!("{}/{}", BASE_URL, ep));
        for (key, val) in map {
            req = req.query(key, val.as_ref()).build(); // this is expensive
        }

        let resp = req
            .set("Client-ID", &self.client_id)
            .set("Accept", "application/vnd.twitchtv.v5+json")
            .timeout_connect(5 * 1000)
            .timeout_read(5 * 1000)
            .call();

        if !resp.ok() {
            warn!("cannot get json for twitch req at {}", ep);
            return Err(Error::HttpGet(ep.to_string()));
        }

        let value: serde_json::Value =
            serde_json::from_reader::<_, serde_json::Value>(resp.into_reader())
                .map_err(|err| Error::Deserialize(Some(err)))?
                .get_mut("data")
                .ok_or_else(|| {
                    error!("cannot get 'data' from json value");
                    Error::Deserialize(None)
                })?
                .take();

        serde_json::from_value(value).map_err(|err| Error::Deserialize(Some(err)))
    }

    pub fn get_names_for<S>(ch: S) -> Result<Names, Error>
    where
        S: AsRef<str>,
    {
        let url = format!("https://tmi.twitch.tv/group/user/{}/chatters", ch.as_ref());
        let names = crate::util::http_get(&url)?;
        Ok(names)
    }
}

#[derive(Debug)]
pub enum Error {
    HttpError(crate::util::HttpError),
    HttpGet(String),
    Deserialize(Option<serde_json::Error>),
}

impl From<crate::util::HttpError> for Error {
    fn from(err: crate::util::HttpError) -> Self {
        Error::HttpError(err)
    }
}

impl From<serde_json::Error> for Error {
    fn from(err: serde_json::Error) -> Self {
        Error::Deserialize(Some(err))
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::HttpError(err) => write!(f, "http error: {}", err),
            Error::HttpGet(ep) => write!(f, "cannot get twitch endpoint: {}", ep),
            Error::Deserialize(Some(err)) => write!(f, "json deserialize error: {}", err),
            Error::Deserialize(None) => write!(f, "missing data field"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::HttpError(err) => Some(err as &dyn std::error::Error),
            Error::HttpGet(..) => None,
            Error::Deserialize(Some(err)) => Some(err as &dyn std::error::Error),
            Error::Deserialize(None) => None,
        }
    }
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
