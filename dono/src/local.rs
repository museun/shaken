use serde::Serialize;

use crate::database;
use crate::error::{Error, Result};
use crate::server;
use crate::FromRow;

#[derive(Serialize)]
pub struct Song {
    pub id: i64,
    pub timestamp: i64,
    pub artist: String,
    pub album: String,
    pub title: String,
}

impl crate::FromRow for Song {
    fn from_row(row: &rusqlite::Row<'_, '_>) -> Self {
        Song {
            id: row.get(0),
            timestamp: row.get(1),
            artist: row.get(2),
            album: row.get(3),
            title: row.get(4),
        }
    }

    fn timestamp(&self) -> i64 {
        self.timestamp
    }
}

pub struct Local;
impl crate::Storage<Song> for Local {
    fn insert(&self, item: &server::Item) -> Result<()> {
        let (title, artist, album) = match &item.kind {
            server::ItemKind::Local {
                title,
                artist,
                album,
            } => (title, artist, album),
            _ => unreachable!("expected a local item"),
        };

        database::get_connection()
            .execute_named(
                include_str!("../sql/local/add_video.sql"),
                &[
                    (":ts", &item.ts),
                    (":title", &title),
                    (":artist", &artist),
                    (":album", &album),
                ],
            )
            .map_err(Error::Sql)
            .map(|_| ())
    }

    fn current(&self) -> Result<Song> {
        database::get_connection()
            .query_row(
                include_str!("../sql/local/get_current.sql"),
                rusqlite::NO_PARAMS,
                Song::from_row,
            )
            .map_err(Error::Sql)
    }

    fn previous(&self) -> Result<Song> {
        database::get_connection()
            .query_row(
                include_str!("../sql/local/get_previous.sql"),
                rusqlite::NO_PARAMS,
                Song::from_row,
            )
            .map_err(Error::Sql)
    }

    fn all(&self) -> Result<Vec<Song>> {
        Ok(database::get_connection()
            .prepare(include_str!("../sql/local/get_all.sql"))?
            .query_map(rusqlite::NO_PARAMS, Song::from_row)
            .map_err(Error::Sql)?
            .filter_map(|s| s.ok())
            .collect())
    }
}
