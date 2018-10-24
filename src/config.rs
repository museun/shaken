use std::{fs, io::Write};

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Config {
    pub twitch: Twitch,
    pub shakespeare: Shakespeare,
    pub invest: Invest,
    pub websocket: WebSocket,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Twitch {
    pub address: String,
    pub port: u32,
    pub name: String,
    pub owners: Vec<i64>,
    pub channel: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Shakespeare {
    pub chance: f64,
    pub bypass: usize,
    pub interval: usize,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Invest {
    pub starting: usize,
    pub line_value: usize,
    pub interval: usize,
    pub chance: f64,
    pub kappas: Vec<[usize; 2]>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct WebSocket {
    pub address: String,
}

const CONFIG_FILE: &str = "shaken.toml"; // hardcoded

impl Default for Config {
    fn default() -> Self {
        Self {
            twitch: Twitch {
                address: "irc.chat.twitch.tv".into(),
                port: 6667,
                name: "shaken_bot".into(),
                owners: vec![23196011],
                channel: "museun".into(), // twitch channel, not irc channel
            },
            shakespeare: Shakespeare {
                interval: 5,
                chance: 0.15,
                bypass: 60,
            },
            invest: Invest {
                starting: 0,
                line_value: 5,
                chance: 1.0 / 2.0,
                interval: 60,
                kappas: vec![[5, 1], [3, 3], [1, 1]],
            },
            websocket: WebSocket {
                address: "localhost:51000".into(),
            },
        }
    }
}

impl Config {
    #[cfg(not(test))]
    pub fn load() -> Self {
        use std::io::ErrorKind;

        let data = fs::read_to_string(CONFIG_FILE)
            .map_err(|e| {
                match e.kind() {
                    ErrorKind::NotFound => {
                        Config::default().save();
                        warn!("created a default config at `{}`", CONFIG_FILE);
                    }
                    ErrorKind::PermissionDenied => {
                        error!("cannot create a config file at `{}`", CONFIG_FILE);
                    }
                    _ => error!("unknown error: {}", e),
                };
                ::std::process::exit(1);
            })
            .unwrap();

        toml::from_str(&data)
            .map_err(|e| {
                error!("unable to parse config: {}", e);
                ::std::process::exit(1);
            })
            .unwrap()
    }

    #[cfg(test)]
    pub fn load() -> Self {
        Config::default()
    }

    fn save(&self) {
        let s = toml::to_string_pretty(&self).expect("to generate correct config");
        let mut f = fs::File::create(CONFIG_FILE)
            .map_err(|e| {
                error!("unable to create config at `{}` -- {}", CONFIG_FILE, e);
                ::std::process::exit(1);
            })
            .unwrap();
        writeln!(f, "{}", s).expect("to write config");
    }
}
