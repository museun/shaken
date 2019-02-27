use log::*;
use serde::Deserialize;

use std::{fmt, fmt::Write, time::Duration};

pub trait CommaSeparated {
    fn commas(&self) -> String;
}

macro_rules! impl_comma {
    (for $($t:ty),+) => {
        $(impl CommaSeparated for $t {
            fn commas(&self) -> String {
                fn comma(n: $t, s: &mut String) {
                    if n < 1000 {
                        write!(s, "{}", n).unwrap();
                        return;
                    }
                    comma(n / 1000, s);
                    write!(s, ",{:03}", n % 1000).unwrap();
                }

                let mut buf = String::new();
                comma(*self, &mut buf);
                buf
            }
        })*
    };
}

impl_comma!(for u64, i64, usize, isize, u32, i32);

pub trait Timestamp {
    fn as_timestamp(&self) -> String;
    fn as_readable_time(&self) -> String;
}

impl Timestamp for Duration {
    fn as_timestamp(&self) -> String {
        let time = self.as_secs();
        let hours = time / (60 * 60);
        let minutes = (time / 60) % 60;
        let seconds = time % 60;

        if hours > 0 {
            format!("{:02}:{:02}:{:02}", hours, minutes, seconds)
        } else {
            format!("{:02}:{:02}", minutes, seconds)
        }
    }

    fn as_readable_time(&self) -> String {
        let table = [
            ("days", 86400), // please
            ("hours", 3600), // dont
            ("minutes", 60), // format
            ("seconds", 1),  // this :(
        ];

        // approx.
        fn plural(s: &str, n: u64) -> String {
            format!("{} {}", n, if n > 1 { s } else { &s[..s.len() - 1] })
        }

        let mut time = vec![];
        let mut secs = self.as_secs();
        for (name, d) in &table {
            let div = secs / d;
            if div > 0 {
                time.push(plural(name, div));
                secs -= d * div;
            }
        }

        let len = time.len();
        if len > 1 {
            if len > 2 {
                for e in &mut time.iter_mut().take(len - 2) {
                    e.push_str(",")
                }
            }
            time.insert(len - 1, "and".into())
        }

        time.join(" ")
    }
}

pub fn http_get_body(url: &str) -> Result<String, HttpError> {
    const FIVE_SECONDS: u64 = 5 * 1000;
    let resp = ureq::get(url)
        .timeout_connect(FIVE_SECONDS)
        .timeout_read(FIVE_SECONDS)
        .call();

    if !resp.ok() {
        warn!("cannot get body for: {}", url);
        return Err(HttpError::HttpGet(url.to_string()));
    }

    let res = resp.into_string()?;
    Ok(res)
}

pub fn http_get_json<T>(url: &str) -> Result<T, HttpError>
where
    for<'de> T: Deserialize<'de>,
{
    const FIVE_SECONDS: u64 = 5 * 1000;
    let resp = ureq::get(url)
        .timeout_connect(FIVE_SECONDS)
        .timeout_read(FIVE_SECONDS)
        .call();

    if !resp.ok() {
        warn!("cannot get body for: {}", url);
        return Err(HttpError::HttpGet(url.to_string()));
    }

    let res = serde_json::from_reader(resp.into_reader())?;
    Ok(res)
}

#[derive(Debug)]
pub enum HttpError {
    HttpGet(String),
    StdIo(std::io::Error),
    Deserialize(serde_json::Error),
}

impl From<std::io::Error> for HttpError {
    fn from(err: std::io::Error) -> Self {
        HttpError::StdIo(err)
    }
}

impl From<serde_json::Error> for HttpError {
    fn from(err: serde_json::Error) -> Self {
        HttpError::Deserialize(err)
    }
}

impl fmt::Display for HttpError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HttpError::HttpGet(url) => write!(f, "cannot get url: {}", url),
            HttpError::StdIo(err) => write!(f, "io error: {}", err),
            HttpError::Deserialize(err) => write!(f, "json deserialize error: {}", err),
        }
    }
}

impl std::error::Error for HttpError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            HttpError::HttpGet(..) => None,
            HttpError::StdIo(err) => Some(err),
            HttpError::Deserialize(err) => Some(err),
        }
    }
}

pub fn get_timestamp() -> u64 {
    use std::time::SystemTime;

    let ts = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap();
    ts.as_secs() * 1000 + u64::from(ts.subsec_nanos()) / 1_000_000
}

pub fn get_log_level(var: &str) -> simplelog::LevelFilter {
    use simplelog::LevelFilter;
    match std::env::var(var)
        .map(|s| s.to_ascii_uppercase())
        .unwrap_or_default()
        .as_str()
    {
        "TRACE" => LevelFilter::Trace,
        "DEBUG" => LevelFilter::Debug,
        "WARN" => LevelFilter::Warn,
        "ERROR" => LevelFilter::Error,

        // default
        "INFO" | _ => LevelFilter::Info,
    }
}

pub fn get_file_size<P>(path: P) -> Option<u64>
where
    P: AsRef<std::path::Path>,
{
    std::fs::metadata(path.as_ref())
        .ok()
        .and_then(|s| Some(s.len() / 1024))
}

#[macro_export]
macro_rules! abort {
    ($f:expr, $($args:expr),* $(,)?) => {{
        let msg = format!($f, $($args),*);
        error!("{}", msg);
        if cfg!(test) {
            panic!("{}", msg);
        }
        ::std::process::exit(1);
    }};
    ($e:expr) => {{
        error!("{}", $e);
        if cfg!(test) {
            panic!("{}", $e);
        }
        ::std::process::exit(1);
    }};
}
