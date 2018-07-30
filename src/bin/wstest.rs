extern crate env_logger;
#[macro_use]
extern crate log;

extern crate curl;
use curl::easy::Easy;

extern crate rand;
use rand::distributions::{Alphanumeric, Standard, Uniform};
use rand::{thread_rng, Rng};

extern crate shaken;
use shaken::*;

use std::thread;
use std::time::Duration;

fn main() {
    env_logger::Builder::from_default_env()
        .default_format_timestamp(false)
        .init();

    let mut env = Environment::new();
    env.config.websocket.address = "localhost:51001".into();

    let state = State::new();
    let _display = Display::new(&env.bot, &env.config);

    loop {
        thread::sleep(Duration::from_millis(500));
        let (user, data) = state.generate();
        env.push_user_context(&user, &data);
        env.step();
        env.drain_msgs();
    }
}

struct State {
    names: Vec<User>,
}

impl State {
    pub fn new() -> Self {
        let names = {
            fn gen_name() -> String {
                let n = thread_rng().sample(Uniform::new(4, 15));
                thread_rng().sample_iter(&Alphanumeric).take(n).collect()
            }

            fn gen_id() -> String {
                let s: String = (0..10)
                    .map(|_| thread_rng().gen_range(0, 9))
                    .map(|s| format!("{}", s))
                    .collect();

                s.trim_left_matches('0').to_string()
            }

            fn gen_color() -> Color {
                let c: Vec<u8> = thread_rng().sample_iter(&Standard).take(3).collect();
                Color::from((c[0], c[1], c[2]))
            }

            (0..10)
                .map(|_f| User {
                    display: gen_name(),
                    userid: gen_id(),
                    color: gen_color(),
                })
                .collect::<Vec<_>>()
        };

        Self { names }
    }

    pub fn generate(&self) -> (&User, String) {
        fn new_message() -> String {
            let mut counter = 0;
            loop {
                if let Some(msg) = http_get("http://localhost:7878/markov/next") {
                    return msg;
                }

                counter += 1;
                warn!(
                    "didn't get a message from the brain, trying again: {}",
                    counter
                )
            }
        }

        let user = thread_rng().choose(&self.names).unwrap();
        (user, new_message())
    }
}

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
