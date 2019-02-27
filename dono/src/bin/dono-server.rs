use log::*;
use serde::{Deserialize, Serialize};
use simplelog::{Config as LogConfig, LevelFilter, TermLogger};

use dono::*;
use error::Error;
use server::HttpServer;

fn get_log_level() -> LevelFilter {
    match std::env::var("DONO_LOG")
        .map(|s| s.to_ascii_uppercase())
        .unwrap_or_default()
        .as_str()
    {
        "TRACE" => LevelFilter::Trace,
        "DEBUG" => LevelFilter::Debug,
        "WARN" => LevelFilter::Warn,
        "ERROR" => LevelFilter::Error,

        // default
        "INFO" | _ => LevelFilter::Info,
    }
}

fn main() {
    TermLogger::init(
        get_log_level(),
        LogConfig::default(), // some config
    )
    .expect("initialize logger");

    let dir = directories::ProjectDirs::from("com.github", "museun", "dono_server").unwrap();
    std::fs::create_dir_all(dir.data_dir()).expect("must be able to create project dirs");
    std::fs::create_dir_all(dir.config_dir()).expect("must be able to create project dirs");

    #[derive(Deserialize, Serialize, Clone, Debug)]
    struct Config {
        pub address: String,
        pub port: u16,
    }

    let file = dir.config_dir().join("config.toml");
    let config: Config = match std::fs::read(&file)
        .ok()
        .and_then(|data| toml::from_slice(&data).ok())
    {
        Some(config) => config,
        None => {
            warn!("creating default config.toml at {}", file.to_str().unwrap());
            warn!("edit and re-run");
            let data = toml::to_string_pretty(&Config {
                address: "localhost".into(),
                port: 50006,
            })
            .expect("valid config");
            std::fs::write(file, &data).expect("write config");
            std::process::exit(1)
        }
    };

    database::DB_PATH
        .set(dir.data_dir().join("videos.db"))
        .expect("must be able to set DB path");

    if let Err(err) = database::get_connection()
        .execute_batch(include_str!("../../sql/schema.sql"))
        .map_err(Error::Sql)
    {
        error!("cannot create tables from schema: {}", err);
        std::process::exit(1)
    }

    let server = match HttpServer::new((config.address.as_str(), config.port)) {
        Ok(server) => server,
        Err(err) => {
            error!("cannot start http server: {}", err);
            std::process::exit(1)
        }
    };

    server.run()
}
