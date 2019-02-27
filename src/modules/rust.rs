use crate::prelude::*;

use log::*;
use serde::Deserialize;

#[derive(Debug)]
enum Error {
    NoMatches,
    CratesGet,
}

pub const NAME: &str = "RustStuff";

submit! {
    template::Response("rust_no_crate", "I couldn't find a crate matching \"${crate}\"");
    template::Response("rust_crate", "${name} = ${max_version} @ ${repo}");
}

pub struct RustStuff {
    map: CommandMap<RustStuff>,
}

impl Module for RustStuff {
    fn command(&mut self, req: &Request) -> Option<Response> {
        let map = self.map.clone();
        map.dispatch(self, req) // why isn't this automatically implemented?
    }
}

impl RustStuff {
    pub fn create() -> Result<Self, ModuleError> {
        Ok(Self {
            map: CommandMap::create(
                "RustStuff",
                &[
                    ("!crate", Self::crates_command),  //
                    ("!crates", Self::crates_command), //
                ],
            )?,
        })
    }
}

impl RustStuff {
    pub fn crates_command(&mut self, req: &Request) -> Option<Response> {
        let query = req.args();
        let c = match Self::lookup_crate(&query) {
            Ok(c) => c,
            Err(Error::NoMatches) => {
                return reply_template!("rust_no_crate", ("crate", &query));
            }
            Err(err) => {
                warn!("cannot look up crate: {} -> {:?}", query, err);
                return None;
            }
        };

        say_template!(
            "rust_crate",
            ("name", &c.name),               //
            ("max_version", &c.max_version), //
            ("repo", &c.repository),         //
        )
    }

    fn lookup_crate(query: &str) -> Result<Crate, Error> {
        let url = format!(
            "https://crates.io/api/v1/crates?page=1&per_page=1&q={}",
            query
        );

        #[derive(Deserialize)]
        struct Resp {
            crates: Vec<Crate>,
        }

        crate::util::http_get::<Resp>(&url)
            .map_err(|_e| Error::CratesGet)
            .and_then(|mut s| s.crates.pop().ok_or_else(|| Error::NoMatches))
    }
}

#[derive(Deserialize)]
struct Crate {
    name: String,
    max_version: String,
    description: String,
    repository: String,
}
