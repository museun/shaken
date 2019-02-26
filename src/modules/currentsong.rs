use crate::prelude::*;

use chrono::prelude::*;
use log::*;
use serde::Deserialize;

pub const NAME: &str = "CurrentSong";

submit! {
    template::Response("currentsong_no_song", "no song is currently playing");
    template::Response("currentsong_youtube_song_delta", "\"${title}\" youtu.be/${vid}?t=${delta}");
    template::Response("currentsong_youtube_song", "\"${title}\" youtu.be/${vid}");
    template::Response("currentsong_unknown_song", "I don't remember a song playing then");
    template::Response("currentsong_previous_song", "previous song: (started at ${start}) \"${title}\" youtu.be/${vid}");
}

pub struct CurrentSong {
    map: CommandMap<CurrentSong>,
}

impl Module for CurrentSong {
    fn command(&mut self, req: &Request) -> Option<Response> {
        let map = self.map.clone();
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
                ],
            )?,
        })
    }

    fn info_command(&mut self, _: &Request) -> Option<Response> {
        let now = Utc::now(); // get this as early as possible

        let song = match Self::single_song(Req::Current) {
            Some(song) => song,
            None => return reply_template!("currentsong_no_song"),
        };

        use chrono::Duration as CDur;
        use std::time::Duration as SDur;

        let start = Utc.timestamp(song.timestamp as i64, 0);
        let dur = CDur::from_std(SDur::from_secs(song.duration as u64)).unwrap();

        let time = dur - (now - start);
        let delta = (dur - time).num_seconds();

        if delta > 0 {
            say_template!(
                "currentsong_youtube_song_delta",
                ("title", &song.title.trim()), //
                ("vid", &song.vid),            //
                ("delta", &delta),             //
            )
        } else {
            info!("not currently playing");
            // TODO maybe note that this isn't actually playing
            say_template!("currentsong_youtube_song", ("vid", &song.vid))
        }
    }

    fn prev_command(&mut self, _: &Request) -> Option<Response> {
        let song = match Self::single_song(Req::Previous) {
            Some(song) => song,
            None => return reply_template!("currentsong_unknown_song"),
        };

        let start = Utc.timestamp(song.timestamp as i64, 0);
        say_template!(
            "currentsong_previous_song",
            ("start", &start),             //
            ("title", &song.title.trim()), //
            ("vid", &song.vid),            //
        )
    }
}

enum Req {
    Current,
    Previous,
}

#[derive(Deserialize)]
struct Song {
    vid: String,
    timestamp: i64,
    duration: i64,
    title: String,
}

#[derive(Debug)]
enum Error {
    YoutubeGet,
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
        let url = format!(
            "http://localhost:50006/{}", // TODO make this configurable
            match req {
                Req::Current => "current",
                Req::Previous => "prev",
            }
        );

        #[derive(Deserialize)]
        struct Resp(Vec<Song>);

        crate::util::http_get::<Resp>(&url)
            .map_err(|_e| Error::YoutubeGet)
            .map(|s| s.0)
    }
}
