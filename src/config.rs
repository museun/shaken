use hashbrown::HashMap;
use log::*;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Config {
    pub enabled: Vec<String>,
    pub twitch: Twitch,
    pub shakespeare: Shakespeare,
    pub invest: Invest,
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
    pub brains: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Invest {
    pub starting: usize,
    pub line_value: usize,
    pub interval: usize,
    pub chance: f64,
    pub kappas: String,
}

impl Default for Config {
    #[allow(clippy::unreadable_literal)]
    fn default() -> Self {
        Self {
            enabled: crate::modules::MODULES
                .iter()
                .cloned() // why
                .map(ToOwned::to_owned)
                .collect::<Vec<_>>(),
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
                brains: vec![],
            },
            invest: Invest {
                starting: 0,
                line_value: 5,
                chance: 1.0 / 2.0,
                interval: 60,
                kappas: "5:1,3:3,1:1".into(),
            },
        }
    }
}

impl Config {
    pub fn env(key: &str) -> Option<String> {
        let map = DotEnvLoader::load(".env").ok()?;
        map.get(key).cloned()
    }

    pub fn expect_env(key: &str) -> String {
        let mut map = DotEnvLoader::load(".env").expect("cannot load env vars");
        map.remove(key)
            .unwrap_or_else(|| abort(format!("env var '{}' not set", key)))
    }

    pub fn load() -> Self {
        if cfg!(test) {
            return Config::default();
        }

        let config_file = get_config_file().unwrap_or_else(|| {
            abort(
                "The system does not have a standard directory for configuration files.\n\naborting",
            );
        });

        let data = std::fs::read_to_string(&config_file).unwrap_or_else(|err| {
            abort(format!(
                "The config file at \"{}\" must be readable.\n{}.\n\naborting",
                config_file.to_string_lossy(),
                err
            ));
        });

        toml::from_str(&data).unwrap_or_else(|e| {
            abort(format!(
                "Unable to parse configuration file at \"{}\".\n{}\n\naborting",
                config_file.to_string_lossy(),
                e
            ));
        })
    }

    pub fn save(&self) {
        if cfg!(test) {
            return;
        }

        let config_file = get_config_file().unwrap_or_else(|| {
            abort("system does not have a standard directory for configuration files. aborting");
        });

        let s = toml::to_string_pretty(&self).expect("generate correct config");
        std::fs::write(&config_file, s).unwrap_or_else(|e| {
            abort(format!(
                "unable to create config at `{}` -- {}",
                config_file.to_string_lossy(),
                e
            ));
        });
    }
}

pub struct DotEnvLoader;
impl DotEnvLoader {
    /// This loads from the path, and overrides the environment with what was
    /// found. this assumes KEY\s?=\s?"?VAL"?\s? and turns it into {KEY:VAL}
    pub fn load(path: impl AsRef<Path>) -> Result<HashMap<String, String>, std::io::Error> {
        fn default_from_env() -> HashMap<String, String> {
            let mut map = HashMap::new();
            for (k, v) in std::env::vars() {
                map.insert(k, v);
            }
            map
        }

        if !check_newer(&path) {
            return Ok(default_from_env());
        }

        let data = match std::fs::read_to_string(path) {
            Ok(data) => data,
            Err(_) => return Ok(default_from_env()),
        };

        let map =
            data.lines()
                .filter(|s| s.starts_with('#'))
                .fold(HashMap::new(), |mut map, line| {
                    let mut line = line.splitn(2, '=').map(|s| s.trim());
                    if let (Some(key), Some(val)) = (line.next(), line.next()) {
                        map.insert(key.into(), val.replace('"', ""));
                    }
                    map
                });

        for (k, v) in &map {
            std::env::set_var(k, v)
        }

        Ok(map)
    }
}

pub fn get_config_file() -> Option<PathBuf> {
    use directories::ProjectDirs;
    ProjectDirs::from("com.github", "museun", "shaken").and_then(|dir| {
        let dir = dir.config_dir();
        std::fs::create_dir_all(&dir)
            .ok()
            .and_then(|_| Some(dir.join("shaken.toml")))
    })
}

fn check_newer(_f: impl AsRef<Path>) -> bool {
    // TODO implement this garbage
    true
}

fn abort<S>(msg: S) -> !
where
    S: AsRef<str>,
{
    error!("{}", msg.as_ref());
    if cfg!(test) {
        panic!("{}", msg.as_ref());
    }
    ::std::process::exit(1);
}
