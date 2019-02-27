use log::*;
use once_cell::sync::Lazy;
use once_cell::sync_lazy;
use regex::Regex;
use serde::{Deserialize, Serialize};

use super::database;
use super::error::{Error, Result};
use super::server;
use super::FromRow;

static PATTERN: Lazy<Regex> = sync_lazy! {
    Regex::new(
        r#"(:?^(:?http?.*?youtu(:?\.be|be.com))(:?/|.*?v=))(?P<id>[A-Za-z0-9_-]{11})"#,
    ).expect("valid regex")
};

static API_KEY: Lazy<String> = sync_lazy! {
    const YOUTUBE_API_KEY: &str = "SHAKEN_YOUTUBE_API_KEY";
    std::env::var(YOUTUBE_API_KEY).map_err(|_| {
        error!("environment var `{}` must be set",YOUTUBE_API_KEY );
        std::process::exit(1);
    }).unwrap()
};

#[derive(Serialize)]
pub struct Song {
    pub id: i64,
    pub vid: String,
    pub timestamp: i64,
    pub duration: i64,
    pub title: String,
}

impl FromRow for Song {
    fn from_row(row: &rusqlite::Row<'_, '_>) -> Self {
        Self {
            id: row.get(0),
            vid: row.get(1),
            timestamp: row.get(2),
            duration: row.get(3),
            title: row.get(4),
        }
    }

    fn timestamp(&self) -> i64 {
        self.timestamp
    }
}

#[derive(Default)]
pub struct Youtube;

impl super::Storage<Song> for Youtube {
    fn insert(&self, item: &server::Item) -> Result<()> {
        let url = match &item.kind {
            server::ItemKind::Youtube(url) => url,
            _ => unreachable!("expected a youtube item"),
        };

        let id = PATTERN
            .captures(&url)
            .and_then(|s| s.name("id"))
            .map(|s| s.as_str())
            .ok_or_else(|| Error::InvalidYoutubeUrl(url.to_string()))?;

        let info = YoutubeItem::fetch(id)?;

        database::get_connection()
            .execute_named(
                include_str!("../sql/youtube/add_video.sql"),
                &[
                    (":vid", &id),
                    (":ts", &item.ts),
                    (":duration", &info.duration),
                    (":title", &info.title),
                ],
            )
            .map_err(Error::Sql)
            .map(|_| ())
    }

    fn current(&self) -> Result<Song> {
        database::get_connection()
            .query_row(
                include_str!("../sql/youtube/get_current.sql"),
                rusqlite::NO_PARAMS,
                Song::from_row,
            )
            .map_err(Error::Sql)
    }

    fn previous(&self) -> Result<Song> {
        database::get_connection()
            .query_row(
                include_str!("../sql/youtube/get_previous.sql"),
                rusqlite::NO_PARAMS,
                Song::from_row,
            )
            .map_err(Error::Sql)
    }

    fn all(&self) -> Result<Vec<Song>> {
        Ok(database::get_connection()
            .prepare(include_str!("../sql/youtube/get_all.sql"))?
            .query_map(rusqlite::NO_PARAMS, Song::from_row)
            .map_err(Error::Sql)?
            .filter_map(|s| s.ok())
            .collect::<Vec<_>>())
    }
}

pub struct YoutubeItem {
    pub title: String,
    pub duration: i64,
}

impl YoutubeItem {
    pub fn fetch(id: &str) -> Result<Self> {
        let mut req = ureq::get("https://www.googleapis.com/youtube/v3/videos");

        for (k, v) in &[
            ("id", id),
            ("part", "snippet,contentDetails"),
            (
                "fields",
                "items(id, snippet(title), contentDetails(duration))",
            ),
            ("key", API_KEY.as_str()),
        ] {
            req = req.set(k, v).build()
        }

        let resp = req.call();
        if let Some(err) = resp.synthetic_error() {
            return Err(Error::HttpResponse(
                err.status(),
                err.status_text().to_string(),
            ));
        }

        #[derive(Deserialize)]
        struct Response {
            items: Vec<Item>,
        }
        #[derive(Deserialize)]
        struct Item {
            snippet: Snippet,
            #[serde(rename = "contentDetails")]
            details: ContentDetails,
        }
        #[derive(Deserialize)]
        struct Snippet {
            title: String,
        }
        #[derive(Deserialize)]
        struct ContentDetails {
            duration: String,
        }

        let data: Response =
            serde_json::from_reader(resp.into_reader()).map_err(Error::Serialize)?;
        let item = &data.items.get(0).ok_or_else(|| Error::InvalidYoutubeData)?;
        Ok(Self {
            title: item.snippet.title.to_string(),
            duration: from_iso8601(&item.details.duration),
        })
    }
}

#[inline]
fn from_iso8601(period: &str) -> i64 {
    let parse = |s, e| period[s + 1..e].parse::<i64>().unwrap_or(0);
    period
        .chars()
        .enumerate()
        .fold((0, 0), |(a, p), (i, c)| match c {
            c if c.is_numeric() => (a, p),
            'H' => (a + parse(p, i) * 60 * 60, i),
            'M' => (a + parse(p, i) * 60, i),
            'S' => (a + parse(p, i), i),
            'P' | 'T' | _ => (a, i),
        })
        .0 as i64
}
