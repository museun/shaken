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
        let mut seconds = self.as_secs();
        let days = seconds / (24 * 60 * 60);
        seconds -= days;
        let hours = seconds / (60 * 60);
        seconds -= hours;
        let minutes = (seconds / 60) % 60;
        seconds -= minutes;
        let seconds = seconds % 60;

        let list = vec![
            (days, "days"),
            (hours, "hours"),
            (minutes, "minutes"),
            (seconds, "seconds"),
        ];

        fn plural(n: u64, s: &str) -> String {
            format!("{} {}", n, if n > 1 { s } else { &s[..s.len() - 1] })
        }

        let mut list = list
            .iter()
            .filter_map(|(n, s)| if *n > 0 { Some(plural(*n, s)) } else { None })
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

        ::util::join_with(list.iter(), " ")
    }
}
