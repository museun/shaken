use std::fs;
use std::io::Write;
use toml;

#[derive(Debug, Deserialize, Serialize)]
pub struct Config {
    pub twitch: Twitch,
    pub shakespeare: Shakespeare,
    pub idlething: IdleThing,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Twitch {
    pub addr: String,
    pub port: u32,
    pub nick: String,
    pub pass: String,
    pub client_id: String,
    pub channels: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Shakespeare {
    pub chance: f64,
    pub bypass: usize,
    pub interval: usize,
}

#[derive(Debug, Deserialize, Serialize)]
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
                addr: "localhost".into(),
                port: 6667,
                pass: env!("TWITCH_PASSWORD").into(),
                client_id: env!("TWITCH_CLIENTID").into(),
                nick: "shaken".into(),
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
            .or_else(|_e| {
                let default = Config::default();
                toml::to_string(&default)
            })
            .unwrap();

        toml::from_str(&data).expect("to parse config")
    }

    pub fn save(&self) {
        let s = toml::to_string_pretty(&self).expect("to generate correct config");
        let mut f = fs::File::create(CONFIG_FILE).expect("to be able to open file");
        writeln!(f, "{}", s).expect("to write config");
    }
}

impl Drop for Config {
    fn drop(&mut self) {
        self.save()
    }
}
