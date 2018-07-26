use std::fs;
use std::io::{ErrorKind, Write};
use toml;

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Config {
    pub twitch: Twitch,
    pub shakespeare: Shakespeare,
    pub idlething: IdleThing,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Twitch {
    pub address: String,
    pub port: u32,
    pub pass: String,
    pub client_id: String,
    pub owners: Vec<String>,
    pub channels: Vec<String>,
    // we don't use a nickname 'cause twitch uses the oauth token for all that
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Shakespeare {
    pub chance: f64,
    pub bypass: usize,
    pub interval: usize,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct IdleThing {
    pub starting: usize,
    pub line_value: usize,
    pub idle_value: usize,
    pub interval: usize,
}

const CONFIG_FILE: &str = "shaken.toml"; // hardcoded

impl Default for Config {
    fn default() -> Self {
        Self {
            twitch: Twitch {
                address: "irc.chat.twitch.tv".into(),
                port: 6667,
                pass: env!("TWITCH_PASSWORD").into(),
                client_id: env!("TWITCH_CLIENTID").into(),
                owners: vec!["23196011".into()],
                channels: vec!["#museun".into()],
            },
            shakespeare: Shakespeare {
                interval: 5,
                chance: 0.15,
                bypass: 60,
            },
            idlething: IdleThing {
                starting: 0,
                line_value: 5,
                idle_value: 1,
                interval: 60,
            },
        }
    }
}

impl Config {
    pub fn load() -> Self {
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
