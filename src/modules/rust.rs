use crate::prelude::*;

use log::*;
use serde::Deserialize;

#[derive(Debug)]
enum Error {
  NoMatches,
  CratesGet,
  Serde(serde_json::error::Error),
}

pub struct RustStuff {
  map: CommandMap<RustStuff>,
}

impl Module for RustStuff {
  fn command(&mut self, req: &Request) -> Option<Response> {
    let map = self.map.shallow_clone();
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
      Err(Error::NoMatches) => return reply!("I couldn't find a crate matching '{}'", query),
      Err(err) => {
        warn!("cannot look up crate: {} -> {:?}", query, err);
        return None;
      }
    };

    say!(
      "{} = {} @ {} \"{}\"",
      c.name,
      c.max_version,
      c.repository,
      c.description.replace("\n", " ").trim_end()
    )
  }

  fn lookup_crate(query: &str) -> Result<Crate, Error> {
    let mut resp = vec![];
    let mut easy = curl::easy::Easy::new();

    let url = format!(
      "https://crates.io/api/v1/crates?page=1&per_page=1&q={}",
      query
    );
    easy.url(&url).map_err(|e| {
      warn!("invalid url: {}", e);
      Error::CratesGet
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
          Error::CratesGet
        })?;

      transfer.perform().map_err(|e| {
        warn!("transfer failed: {}", e);
        Error::CratesGet
      })?;
    }

    #[derive(Deserialize)]
    struct Resp {
      crates: Vec<Crate>,
    }

    trace!("{}", String::from_utf8_lossy(&resp));

    serde_json::from_slice::<Resp>(&resp)
      .map_err(Error::Serde)
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
