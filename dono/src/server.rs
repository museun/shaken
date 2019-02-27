use std::fmt;
use std::net::ToSocketAddrs;

use log::*;
use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::Storage;

use crate::{local::Local, youtube::Youtube};

pub struct HttpServer {
    server: tiny_http::Server,
    list_regex: Regex,
}

impl HttpServer {
    pub fn new<A>(addr: A) -> Result<Self>
    where
        A: ToSocketAddrs + fmt::Debug + Clone,
    {
        let server = tiny_http::Server::http(addr.clone()).map_err(|err| {
            error!("cannot bind http server at {:?}: {}", addr, err);
            Error::BindHttp(format!("{:?}", addr))
        })?;

        info!(
            "started http server at: {}",
            addr.to_socket_addrs().unwrap().next().unwrap()
        );

        Ok(Self {
            server,
            list_regex: Regex::new(r#"/list/(?P<ty>\w.*?)(/|$)"#).expect("regex"),
        })
    }

    pub fn run(mut self) {
        loop {
            let req = match self.server.recv() {
                Ok(req) => req,
                Err(err) => {
                    error!("cannot recv request: {}", err);
                    continue;
                }
            };

            if let Err(err) = self.handle(req) {
                error!("processing request failed: {}", err)
            }
        }
    }

    fn handle(&mut self, mut req: tiny_http::Request) -> Result<()> {
        trace!("{} {}", req.method(), req.url());

        macro_rules! err {
            ($req:expr) => {{
                debug!("unknown {} on {}", $req.method(), req.url());
                $req.respond(tiny_http::Response::empty(400))
                    .map_err(Error::Io)
            }};
        }

        use tiny_http::Method::*;

        match (req.method(), req.url()) {
            (Get, "/current") => Self::compare(
                Youtube.current().map(|t| (t, Kind::Youtube)),
                Local.current().map(|t| (t, Kind::Local)),
                req,
                std::cmp::Ordering::Greater,
            ),
            (Get, "/previous") => Self::compare(
                Youtube.current().map(|t| (t, Kind::Youtube)),
                Local.current().map(|t| (t, Kind::Local)),
                req,
                std::cmp::Ordering::Less,
            ),
            (Get, other) => {
                let namespace = self
                    .list_regex
                    .captures(other)
                    .and_then(|c| c.name("ty"))
                    .and_then(|s| Some(s.as_str()))
                    .map(str::to_lowercase);

                match namespace.unwrap_or_else(|| "".into()).as_str() {
                    "youtube" => Self::respond(Self::check(Youtube.all(), req)?),
                    "local" => Self::respond(Self::check(Local.all(), req)?),
                    _ => err!(req),
                }
            }

            (Post, path @ "/youtube") | (Post, path @ "/local") => {
                trace!("handling post at {}", path);

                // TODO return a better error message for this (wrong version, etc)
                let item: Item =
                    serde_json::from_reader(req.as_reader()).map_err(Error::Serialize)?;

                if item.version != 1 {
                    return req
                        .respond(tiny_http::Response::empty(400))
                        .map_err(Error::Io);
                }

                match item.kind {
                    ItemKind::Local { .. } => Local.insert(&item),
                    ItemKind::Youtube(..) => Youtube.insert(&item),
                }
            }

            _ => err!(req),
        }
    }

    fn compare<L, R>(
        left: Result<(L, Kind)>,
        right: Result<(R, Kind)>,
        req: tiny_http::Request,
        op: std::cmp::Ordering,
    ) -> Result<()>
    where
        L: Serialize + crate::FromRow,
        R: Serialize + crate::FromRow,
    {
        #[derive(Serialize)]
        #[serde(rename_all = "lowercase")]
        struct Outgoing<T>
        where
            T: Serialize + crate::FromRow,
        {
            data: T,
            kind: Kind,
        }

        impl<T> From<(T, Kind)> for Outgoing<T>
        where
            T: Serialize + crate::FromRow,
        {
            fn from((data, kind): (T, Kind)) -> Self {
                Self { data, kind }
            }
        }

        let left = left.map(Outgoing::from);
        let right = right.map(Outgoing::from);

        match (left.is_err(), right.is_err()) {
            (true, true) => {
                warn!("no songs in either table");
                req.respond(tiny_http::Response::from_string("[]").with_status_code(204))
                    .map_err(Error::Io)
            }
            (false, true) => Self::respond((vec![left.unwrap()], req)),
            (true, false) => Self::respond((vec![right.unwrap()], req)),
            (false, false) => {
                let (left, right) = (left.unwrap(), right.unwrap());
                if left.data.timestamp().cmp(&right.data.timestamp()) == op {
                    Self::respond((vec![left], req))
                } else {
                    Self::respond((vec![right], req))
                }
            }
        }
    }

    fn check<T>(res: Result<T>, req: tiny_http::Request) -> Result<(T, tiny_http::Request)> {
        match res {
            Ok(d) => Ok((d, req)),
            Err(err) => req
                .respond(tiny_http::Response::empty(500))
                .map_err(Error::Io)
                .and_then(|_| Err(err)),
        }
    }

    fn respond<T>((res, req): (T, tiny_http::Request)) -> Result<()>
    where
        T: Serialize,
    {
        let data = match serde_json::to_vec(&res).map_err(Error::Serialize) {
            Ok(data) => data,
            Err(err) => {
                return req
                    .respond(tiny_http::Response::empty(400))
                    .map_err(Error::Io)
                    .and_then(|_| Err(err));
            }
        };
        req.respond(tiny_http::Response::from_data(data))
            .map_err(Error::Io)
    }
}

#[derive(Serialize)]
#[serde(rename_all = "lowercase")]
enum Kind {
    Youtube,
    Local,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ItemKind {
    Youtube(String),
    Local {
        artist: String,
        title: String,
        album: String,
    },
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub struct Item {
    pub kind: ItemKind,
    pub ts: i64,
    pub version: u32,
}
