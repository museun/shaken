use std::fs;
use std::io::Write;
use toml;

#[derive(Debug, Deserialize, Serialize)]
pub struct Config {
    pub addr: String,
    pub port: u32,
    pub nick: String,
    pub pass: String,
    pub channels: Vec<String>,
    pub chance: f64,
    pub interval: usize,
}

const CONFIG_FILE: &str = "shaken.toml"; // hardcoded

impl Config {
    pub fn load() -> Self {
        let data = fs::read_to_string(CONFIG_FILE)
            .or_else(|_e| {
                let default = Config {
                    addr: "localhost".into(),
                    port: 6667,
                    pass: env!("TWITCH_PASSWORD").into(),
                    nick: "shaken".into(),
                    channels: vec!["#museun".into()],
                    interval: 5,
                    chance: 0.15,
                };
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
