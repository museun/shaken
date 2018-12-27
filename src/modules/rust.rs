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

/*

{
  "crates": [
    {
      "id": "tokio",
      "name": "tokio",
      "updated_at": "2018-11-22T01:24:35.398065+00:00",
      "versions": null,
      "keywords": null,
      "categories": null,
      "badges": [
        {
          "badge_type": "travis-ci",
          "attributes": {
            "branch": null,
            "repository": "tokio-rs/tokio"
          }
        },
        {
          "badge_type": "appveyor",
          "attributes": {
            "project_name": null,
            "repository": "carllerche/tokio",
            "branch": null,
            "id": "s83yxhy9qeb58va7",
            "service": null
          }
        }
      ],
      "created_at": "2016-07-01T20:39:07.497766+00:00",
      "downloads": 898637,
      "recent_downloads": 387916,
      "max_version": "0.1.13",
      "description": "An event-driven, non-blocking I/O platform for writing asynchronous I/O\nbacked applications.\n",
      "homepage": "https://tokio.rs",
      "documentation": "https://docs.rs/tokio/0.1.13/tokio/",
      "repository": "https://github.com/tokio-rs/tokio",
      "links": {
        "version_downloads": "/api/v1/crates/tokio/downloads",
        "versions": "/api/v1/crates/tokio/versions",
        "owners": "/api/v1/crates/tokio/owners",
        "owner_team": "/api/v1/crates/tokio/owner_team",
        "owner_user": "/api/v1/crates/tokio/owner_user",
        "reverse_dependencies": "/api/v1/crates/tokio/reverse_dependencies"
      },
      "exact_match": true
    },
    {
      "id": "new-tokio-smtp",
      "name": "new-tokio-smtp",
      "updated_at": "2018-11-06T10:13:04.854115+00:00",
      "versions": null,
      "keywords": null,
      "categories": null,
      "badges": [],
      "created_at": "2018-08-22T18:06:08.123170+00:00",
      "downloads": 388,
      "recent_downloads": 347,
      "max_version": "0.8.1",
      "description": "extendible smtp implementation for tokio",
      "homepage": null,
      "documentation": "https://docs.rs/new-tokio-smtp",
      "repository": "https://github.com/1aim/new-tokio-smtp",
      "links": {
        "version_downloads": "/api/v1/crates/new-tokio-smtp/downloads",
        "versions": "/api/v1/crates/new-tokio-smtp/versions",
        "owners": "/api/v1/crates/new-tokio-smtp/owners",
        "owner_team": "/api/v1/crates/new-tokio-smtp/owner_team",
        "owner_user": "/api/v1/crates/new-tokio-smtp/owner_user",
        "reverse_dependencies": "/api/v1/crates/new-tokio-smtp/reverse_dependencies"
      },
      "exact_match": false
    },
    {
      "id": "tokio-imap",
      "name": "tokio-imap",
      "updated_at": "2018-11-06T09:03:48.073391+00:00",
      "versions": null,
      "keywords": null,
      "categories": null,
      "badges": [
        {
          "badge_type": "travis-ci",
          "attributes": {
            "repository": "djc/tokio-imap",
            "branch": null
          }
        }
      ],
      "created_at": "2017-06-10T19:55:16.035147+00:00",
      "downloads": 604,
      "recent_downloads": 215,
      "max_version": "0.4.0",
      "description": "Tokio-based IMAP protocol (client, for now) implementation",
      "homepage": "https://github.com/djc/tokio-imap",
      "documentation": "https://docs.rs/tokio-imap",
      "repository": "https://github.com/djc/tokio-imap",
      "links": {
        "version_downloads": "/api/v1/crates/tokio-imap/downloads",
        "versions": "/api/v1/crates/tokio-imap/versions",
        "owners": "/api/v1/crates/tokio-imap/owners",
        "owner_team": "/api/v1/crates/tokio-imap/owner_team",
        "owner_user": "/api/v1/crates/tokio-imap/owner_user",
        "reverse_dependencies": "/api/v1/crates/tokio-imap/reverse_dependencies"
      },
      "exact_match": false
    },
    {
      "id": "tokio-io-pool",
      "name": "tokio-io-pool",
      "updated_at": "2018-12-04T18:51:04.526433+00:00",
      "versions": null,
      "keywords": null,
      "categories": null,
      "badges": [
        {
          "badge_type": "maintenance",
          "attributes": {
            "status": "experimental"
          }
        },
        {
          "badge_type": "travis-ci",
          "attributes": {
            "branch": null,
            "repository": "jonhoo/tokio-io-pool"
          }
        }
      ],
      "created_at": "2018-07-12T17:10:54.369021+00:00",
      "downloads": 9391,
      "recent_downloads": 8824,
      "max_version": "0.1.5",
      "description": "Alternative tokio thread pool for I/O-heavy applications",
      "homepage": "https://github.com/jonhoo/tokio-io-pool",
      "documentation": null,
      "repository": "https://github.com/jonhoo/tokio-io-pool.git",
      "links": {
        "version_downloads": "/api/v1/crates/tokio-io-pool/downloads",
        "versions": "/api/v1/crates/tokio-io-pool/versions",
        "owners": "/api/v1/crates/tokio-io-pool/owners",
        "owner_team": "/api/v1/crates/tokio-io-pool/owner_team",
        "owner_user": "/api/v1/crates/tokio-io-pool/owner_user",
        "reverse_dependencies": "/api/v1/crates/tokio-io-pool/reverse_dependencies"
      },
      "exact_match": false
    },
    {
      "id": "tokio-modbus",
      "name": "tokio-modbus",
      "updated_at": "2018-12-03T12:38:25.860390+00:00",
      "versions": null,
      "keywords": null,
      "categories": null,
      "badges": [
        {
          "badge_type": "coveralls",
          "attributes": {
            "branch": "master",
            "service": "github",
            "repository": "slowtec/tokio-modbus"
          }
        },
        {
          "badge_type": "travis-ci",
          "attributes": {
            "repository": "slowtec/tokio-modbus",
            "branch": null
          }
        },
        {
          "badge_type": "maintenance",
          "attributes": {
            "status": "actively-developed"
          }
        }
      ],
      "created_at": "2017-08-21T11:13:36.566980+00:00",
      "downloads": 900,
      "recent_downloads": 475,
      "max_version": "0.2.3",
      "description": "Tokio-based modbus library",
      "homepage": "https://github.com/slowtec/tokio-modbus",
      "documentation": null,
      "repository": "https://github.com/slowtec/tokio-modbus",
      "links": {
        "version_downloads": "/api/v1/crates/tokio-modbus/downloads",
        "versions": "/api/v1/crates/tokio-modbus/versions",
        "owners": "/api/v1/crates/tokio-modbus/owners",
        "owner_team": "/api/v1/crates/tokio-modbus/owner_team",
        "owner_user": "/api/v1/crates/tokio-modbus/owner_user",
        "reverse_dependencies": "/api/v1/crates/tokio-modbus/reverse_dependencies"
      },
      "exact_match": false
    },
    {
      "id": "tokio-zmq",
      "name": "tokio-zmq",
      "updated_at": "2018-12-23T16:12:58.888713+00:00",
      "versions": null,
      "keywords": null,
      "categories": null,
      "badges": [],
      "created_at": "2018-01-02T02:49:18.380278+00:00",
      "downloads": 3930,
      "recent_downloads": 1682,
      "max_version": "0.9.0",
      "description": "Provides Futures abstractions for ZeroMQ on the Tokio event-loop",
      "homepage": null,
      "documentation": null,
      "repository": "https://git.asonix.dog/asonix/async-zmq",
      "links": {
        "version_downloads": "/api/v1/crates/tokio-zmq/downloads",
        "versions": "/api/v1/crates/tokio-zmq/versions",
        "owners": "/api/v1/crates/tokio-zmq/owners",
        "owner_team": "/api/v1/crates/tokio-zmq/owner_team",
        "owner_user": "/api/v1/crates/tokio-zmq/owner_user",
        "reverse_dependencies": "/api/v1/crates/tokio-zmq/reverse_dependencies"
      },
      "exact_match": false
    },
    {
      "id": "tokio-http2",
      "name": "tokio-http2",
      "updated_at": "2017-01-18T15:35:46.283749+00:00",
      "versions": null,
      "keywords": null,
      "categories": null,
      "badges": [],
      "created_at": "2016-12-04T16:31:45.868366+00:00",
      "downloads": 2079,
      "recent_downloads": 388,
      "max_version": "0.1.9",
      "description": "HTTP/1.1 Library (HTTP/2 coming soon) using Tokio Project (core, proto, service). Used with https://github.com/lambdastackio/httpd.\n",
      "homepage": "https://lambdastackio.github.io/tokio-http2/tokio_http2",
      "documentation": "https://lambdastackio.github.io/tokio-http2/tokio_http2",
      "repository": "https://github.com/lambdastackio/tokio-http2",
      "links": {
        "version_downloads": "/api/v1/crates/tokio-http2/downloads",
        "versions": "/api/v1/crates/tokio-http2/versions",
        "owners": "/api/v1/crates/tokio-http2/owners",
        "owner_team": "/api/v1/crates/tokio-http2/owner_team",
        "owner_user": "/api/v1/crates/tokio-http2/owner_user",
        "reverse_dependencies": "/api/v1/crates/tokio-http2/reverse_dependencies"
      },
      "exact_match": false
    },
    {
      "id": "tokio-file-futures",
      "name": "tokio-file-futures",
      "updated_at": "2018-05-02T23:06:32.151438+00:00",
      "versions": null,
      "keywords": null,
      "categories": null,
      "badges": [],
      "created_at": "2018-05-02T23:06:32.151438+00:00",
      "downloads": 112,
      "recent_downloads": 50,
      "max_version": "0.1.0",
      "description": "Some basic futures on top of tokio-fs's polled file operations",
      "homepage": null,
      "documentation": null,
      "repository": "https://github.com/asonix/file-futures",
      "links": {
        "version_downloads": "/api/v1/crates/tokio-file-futures/downloads",
        "versions": "/api/v1/crates/tokio-file-futures/versions",
        "owners": "/api/v1/crates/tokio-file-futures/owners",
        "owner_team": "/api/v1/crates/tokio-file-futures/owner_team",
        "owner_user": "/api/v1/crates/tokio-file-futures/owner_user",
        "reverse_dependencies": "/api/v1/crates/tokio-file-futures/reverse_dependencies"
      },
      "exact_match": false
    },
    {
      "id": "tokio-rpc",
      "name": "tokio-rpc",
      "updated_at": "2017-04-03T08:44:10.655437+00:00",
      "versions": null,
      "keywords": null,
      "categories": null,
      "badges": [],
      "created_at": "2017-04-01T12:07:16.949791+00:00",
      "downloads": 506,
      "recent_downloads": 122,
      "max_version": "0.1.1",
      "description": "An RPC framework for Rust base on tokio.",
      "homepage": null,
      "documentation": null,
      "repository": "https://github.com/iorust/tokio-rpc",
      "links": {
        "version_downloads": "/api/v1/crates/tokio-rpc/downloads",
        "versions": "/api/v1/crates/tokio-rpc/versions",
        "owners": "/api/v1/crates/tokio-rpc/owners",
        "owner_team": "/api/v1/crates/tokio-rpc/owner_team",
        "owner_user": "/api/v1/crates/tokio-rpc/owner_user",
        "reverse_dependencies": "/api/v1/crates/tokio-rpc/reverse_dependencies"
      },
      "exact_match": false
    },
    {
      "id": "tokio-file",
      "name": "tokio-file",
      "updated_at": "2018-11-29T17:31:52.398045+00:00",
      "versions": null,
      "keywords": null,
      "categories": null,
      "badges": [],
      "created_at": "2017-07-26T03:34:41.304140+00:00",
      "downloads": 557,
      "recent_downloads": 237,
      "max_version": "0.5.0",
      "description": "Asynchronous file I/O for Tokio\n",
      "homepage": null,
      "documentation": "https://asomers.github.io/tokio-file/tokio_file/",
      "repository": "https://github.com/asomers/tokio-file",
      "links": {
        "version_downloads": "/api/v1/crates/tokio-file/downloads",
        "versions": "/api/v1/crates/tokio-file/versions",
        "owners": "/api/v1/crates/tokio-file/owners",
        "owner_team": "/api/v1/crates/tokio-file/owner_team",
        "owner_user": "/api/v1/crates/tokio-file/owner_user",
        "reverse_dependencies": "/api/v1/crates/tokio-file/reverse_dependencies"
      },
      "exact_match": false
    }
  ],
  "meta": {
    "total": 424
  }
}
*/
