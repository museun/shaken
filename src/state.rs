use std::time;

use curl::easy::Easy;

pub struct State {
    pub(crate) previous: Option<time::Instant>,
    pub(crate) limit: time::Duration,
    pub(crate) interval: f64,
    pub(crate) chance: f64,
}

impl State {
    pub fn new(interval: usize, chance: f64) -> Self {
        State {
            previous: None,
            limit: time::Duration::from_secs(interval as u64),
            interval: interval as f64,
            chance,
        }
    }

    pub fn generate(&mut self) -> Option<String> {
        let now = time::Instant::now();
        if let Some(prev) = self.previous {
            if now.duration_since(prev) < self.limit {
                let dur = now.duration_since(prev);
                let rem =
                    self.interval - (dur.as_secs() as f64 + f64::from(dur.subsec_nanos()) * 1e-9);
                debug!("already spoke: {:.3}s remaining", rem);
                None?;
            }
        }

        if let Some(data) = get("http://localhost:7878/markov/next") {
            trace!("generated a message");
            self.previous = Some(now);
            Some(prune(&data).to_string() + ".")
        } else {
            None
        }
    }
}

fn prune(s: &str) -> &str {
    let mut pos = 0;
    for c in s.chars().rev() {
        if c.is_alphabetic() {
            break;
        }
        pos += 1
    }
    &s[..s.len() - pos]
}

fn get(url: &str) -> Option<String> {
    let mut vec = Vec::new();
    let mut easy = Easy::new();
    easy.url(url).ok()?;
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
