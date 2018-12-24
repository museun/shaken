use crate::prelude::*;

use chrono::prelude::*;
use log::*;
use serde::Deserialize;

pub struct CurrentSong {
    map: CommandMap<CurrentSong>,
    paste: Option<String>,
    dirty: bool,
}

impl Module for CurrentSong {
    fn command(&mut self, req: &Request) -> Option<Response> {
        let map = self.map.shallow_clone();
        map.dispatch(self, req) // why isn't this automatically implemented?
    }
}

impl CurrentSong {
    pub fn create() -> Result<Self, ModuleError> {
        Ok(Self {
            map: CommandMap::create(
                "CurrentSong",
                &[
                    ("!song", Self::info_command),     //
                    ("!prevsong", Self::prev_command), //
                    ("!songlist", Self::list_command), //
                ],
            )?,
            paste: None,
            dirty: true,
        })
    }

    fn info_command(&mut self, _: &Request) -> Option<Response> {
        let now = Utc::now(); // get this as early as possible

        let song = match Self::single_song(Req::Current) {
            Some(song) => song,
            None => return reply!("no song is currently playing"),
        };

        use chrono::Duration as CDur;
        use std::time::Duration as SDur;

        let start = Utc.timestamp(song.timestamp as i64, 0);
        let dur = CDur::from_std(SDur::from_secs(song.duration as u64)).unwrap();

        let time = dur - (now - start);
        let delta = (dur - time).num_seconds();

        if delta > 0 {
            say!(
                "\"{}\" youtu.be/{}?t={}s",
                song.title.trim(),
                song.vid,
                delta
            )
        } else {
            info!("not currently playing");
            // TODO maybe note that this isn't actually playing
            say!("\"{}\" youtu.be/{}", song.title.trim(), song.vid)
        }
    }

    fn prev_command(&mut self, _: &Request) -> Option<Response> {
        let song = match Self::single_song(Req::Previous) {
            Some(song) => song,
            None => return reply!("I don't remember a song playing then"),
        };

        let start = Utc.timestamp(song.timestamp as i64, 0);
        say!(
            "previous song: (started at {}) \"{}\" youtu.be/{}",
            start,
            song.title.trim(),
            song.vid
        )
    }

    fn list_command(&mut self, _: &Request) -> Option<Response> {
        let songs = match Self::get_songs(Req::List) {
            Ok(songs) => songs,
            Err(..) => return reply!("I don't have any songs listed"),
        };

        match self.paste_list(&songs) {
            Ok(link) => say!("{}", link),
            Err(..) => reply!("I couldn't make a paste of the songs"),
        }
    }
}

enum Req {
    Current,
    Previous,
    List,
}

#[derive(Deserialize)]
struct Song {
    id: i64,
    vid: String,
    timestamp: i64,
    duration: i64,
    title: String,
}

#[derive(Debug)]
enum Error {
    YoutubeGet,
    IxPaste,
    Serde(serde_json::error::Error),
}

impl CurrentSong {
    fn single_song(req: Req) -> Option<Song> {
        match Self::get_songs(req) {
            Err(err) => {
                debug!("error getting song: {:?}", err);
                None
            }
            Ok(mut songs) => songs.pop(),
        }
    }

    fn get_songs(req: Req) -> Result<Vec<Song>, Error> {
        let mut resp = vec![];
        let mut easy = curl::easy::Easy::new();

        let url = format!(
            "http://localhost:50006/{}",
            match req {
                Req::Current => "current",
                Req::Previous => "prev",
                Req::List => "list",
            }
        );
        easy.url(&url).map_err(|e| {
            warn!("invalid url: {}", e);
            Error::YoutubeGet
        })?;

        {
            let mut transfer = easy.transfer();
            transfer
                .write_function(|d| {
                    resp.extend_from_slice(&d);
                    Ok(d.len())
                })
                .map_err(|e| {
                    warn!("write function failed: {}", e);
                    Error::YoutubeGet
                })?;

            transfer.perform().map_err(|e| {
                warn!("transfer failed: {}", e);
                Error::YoutubeGet
            })?;
        }

        #[derive(Deserialize)]
        struct Resp(Vec<Song>);

        trace!("{}", String::from_utf8_lossy(&resp));

        serde_json::from_slice::<Resp>(&resp)
            .map_err(Error::Serde)
            .map(|s| s.0)
    }

    fn paste_list(&mut self, songs: &[Song]) -> Result<String, Error> {
        if !self.dirty && !songs.is_empty() {
            return self.paste.clone().ok_or_else(|| Error::IxPaste); // shouldn't happen
        }

        let out = songs.iter().rev().map(|song| {
            let start = Utc.timestamp(song.timestamp as i64, 0);
            format!(
                "#{}\t{}\nlink\thttps://www.youtube.com/watch?v={}\nat\t{}\n\n", //
                song.id, song.title, song.vid, start
            )
        });

        use curl::easy::{Easy, Form};
        let mut easy = Easy::new();
        easy.url("http://ix.io").map_err(|e| {
            warn!("invalid ix.io url: {}", e);
            Error::IxPaste
        })?;

        let mut form = Form::new();
        form.part("f:1")
            .contents(
                &out.fold(String::new(), |mut a, c| {
                    a.push_str(&c);
                    a
                })
                .as_bytes(),
            )
            .add()
            .map_err(|e| {
                warn!("invalid form: {}", e);
                Error::IxPaste
            })?;

        easy.httppost(form).map_err(|e| {
            warn!("cannot set post for form: {}", e);
            Error::IxPaste
        })?;

        let mut data = vec![];
        {
            let mut transfer = easy.transfer();
            transfer
                .write_function(|d| {
                    data.extend_from_slice(&d);
                    Ok(d.len())
                })
                .map_err(|e| {
                    warn!("write function failed: {}", e);
                    Error::IxPaste
                })?;

            transfer.perform().map_err(|e| {
                warn!("transfer failed: {}", e);
                Error::IxPaste
            })?;
        }

        self.dirty = false;
        let resp = String::from_utf8_lossy(&data);
        self.paste.replace(resp.into());
        self.paste.clone().ok_or_else(|| Error::IxPaste) // shouldn't happen
    }
}
