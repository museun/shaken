use std::fmt::Write;
use std::fs;
use std::path::Path;
use std::time::Instant;

#[macro_export]
macro_rules! timeit {
    ($($arg:tt)+) => {
        let data = format!($($arg)+);
        eprintln!("[{: ^46}]", data);
        let _time = $crate::Measure::new();
    };
}

pub struct Measure {
    start: Instant,
}

impl Measure {
    pub fn new() -> Self {
        Measure {
            start: Instant::now(),
        }
    }
}

impl Default for Measure {
    fn default() -> Self {
        Measure::new()
    }
}

impl Drop for Measure {
    fn drop(&mut self) {
        let ms = (self.start.elapsed().as_secs() as f64 * 1_000.0)
            + (f64::from(self.start.elapsed().subsec_nanos()) / 1_000_000.0);

        let time = match ms as u64 {
            0..=3000 => format!("{:.3}ms", ms),
            3001..=60000 => format!("{:.2}s", ms / 1000.0),
            _ => format!("{:.2}m", ms / 1000.0 / 60.0),
        };

        println!("{: >48}", time);
        println!("{:-<48}", "");
    }
}

pub fn get_file_size(path: impl AsRef<Path>) -> Option<u64> {
    fs::metadata(path.as_ref())
        .ok()
        .and_then(|s| Some(s.len() / 1024))
}

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
