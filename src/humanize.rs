use std::fmt::Write;
use std::time::Duration;

pub trait CommaSeparated {
    fn comma_separate(&self) -> String;
}

macro_rules! impl_comma {
    (for $($t:ty),+) => {
        $(impl CommaSeparated for $t {
            fn comma_separate(&self) -> String {
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

pub(crate) fn format_time_map<S, V>(list: V) -> String
where
    S: AsRef<str>,
    V: AsRef<[(S, u64)]>,
{
    fn plural(n: u64, s: &str) -> String {
        format!("{} {}", n, if n > 1 { s } else { &s[..s.len() - 1] })
    }

    let mut list = list
        .as_ref()
        .iter()
        .filter_map(|(s, n)| {
            if *n > 0 {
                Some(plural(*n, s.as_ref()))
            } else {
                None
            }
        })
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

    crate::util::join_with(list.iter(), " ")
}
