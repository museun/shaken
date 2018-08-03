#![allow(dead_code)]
use curl::easy::Easy;

pub fn http_get<S: AsRef<str>>(url: S) -> Option<String> {
    let mut vec = Vec::new();
    let mut easy = Easy::new();
    easy.url(url.as_ref()).ok()?;
    {
        let mut transfer = easy.transfer();
        let _ = transfer.write_function(|data| {
            vec.extend_from_slice(data);
            Ok(data.len())
        });
        transfer.perform().ok()?;
    }
    String::from_utf8(vec).ok()
}

pub fn join_with<S, I, T>(mut iter: I, sep: S) -> String
where
    S: AsRef<str>,
    T: AsRef<str>,
    I: Iterator<Item = T>,
{
    let mut buf = String::new();
    if let Some(s) = iter.next() {
        buf.push_str(s.as_ref());
    }
    for i in iter {
        buf.push_str(sep.as_ref());
        buf.push_str(i.as_ref());
    }
    buf
}

pub fn get_timestamp() -> u64 {
    use std::time::SystemTime;

    let ts = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap();
    ts.as_secs() * 1000 + u64::from(ts.subsec_nanos()) / 1_000_000
}

#[macro_export]
macro_rules! bail {
    ($e:expr) => {
        match $e {
            Some(item) => item,
            None => return,
        }
    };
}
