use curl::easy::Easy;
use std::{fmt::Write, time::Duration};

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

        let mut time = vec![];
        let mut secs = self.as_secs();
        for (name, d) in &table {
            let div = secs / d;
            if div > 0 {
                time.push((name, div));
                secs -= d * div;
            }
        }

        format_time_map(&time)
    }
}

pub fn format_time_map<S, V>(list: V) -> String
where
    S: AsRef<str>,
    V: AsRef<[(S, u64)]>,
{
    fn plural((s, n): (&str, u64)) -> String {
        format!("{} {}", n, if n > 1 { s } else { &s[..s.len() - 1] })
    }

    let mut list = list
        .as_ref()
        .iter()
        .map(|(s, n)| (s.as_ref(), *n))
        .filter(|&(_, n)| n > 0)
        .map(plural)
        .collect::<Vec<_>>();

    let len = list.len();
    if len > 1 {
        if len > 2 {
            for e in &mut list.iter_mut().take(len - 2) {
                e.push_str(",")
            }
        }
        list.insert(len - 1, "and".into())
    }

    join_with(list.iter(), " ")
}

pub fn http_get<S: AsRef<str>>(url: S) -> Option<String> {
    let mut vec = Vec::new();
    let mut easy = Easy::new();
    easy.connect_timeout(::std::time::Duration::from_secs(5))
        .expect("to set timeout");
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
