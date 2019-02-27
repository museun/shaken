use std::fmt;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug)]
pub enum Error {
    Io(std::io::Error),
    Sql(rusqlite::Error),
    Deserialize(serde_json::Error),
    Serialize(serde_json::Error),
    HttpResponse(u16, String),
    BindHttp(String),

    InvalidYoutubeUrl(String),
    InvalidYoutubeData, // context?
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Error::Io(err) => write!(f, "io error: {}", err),
            Error::Sql(err) => write!(f, "sql error: {}", err),
            Error::Deserialize(err) => write!(f, "deserialization error: {}", err),
            Error::Serialize(err) => write!(f, "serialization error: {}", err),
            Error::HttpResponse(code, reason) => {
                write!(f, "http get failed: ({}) {}", code, reason)
            }
            Error::BindHttp(addr) => write!(f, "cannot bind http server to {}", addr),
            Error::InvalidYoutubeUrl(url) => write!(f, "invalid youtube url: {}", url),
            Error::InvalidYoutubeData => write!(f, "missing snippet from youtube response"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        macro_rules! cast {
            ($e:expr) => {
                Some($e as &(dyn std::error::Error + 'static))
            };
        }

        match self {
            Error::Io(err) => cast!(err),
            Error::Sql(err) => cast!(err),
            Error::Deserialize(err) | Error::Serialize(err) => cast!(err),
            _ => None,
        }
    }
}

impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Self {
        Error::Io(err)
    }
}

impl From<rusqlite::Error> for Error {
    fn from(err: rusqlite::Error) -> Self {
        Error::Sql(err)
    }
}
