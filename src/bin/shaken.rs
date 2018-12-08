use crossbeam_channel as channel;
use log::{error, info, warn};
use scoped_threadpool::Pool;
use simplelog::{Config as LogConfig, LevelFilter, TermLogger};

use std::sync::{Arc, Mutex};
use std::{env, thread::sleep, time};

use shaken::modules::*;
use shaken::prelude::*;

fn main() {
    TermLogger::init(
        get_log_level(),      // log level
        LogConfig::default(), // some config
    )
    .expect("initialize logger");

    let config = Config::load();
    let address = format!("{}:{}", &config.twitch.address, &config.twitch.port);

    let mut delay = 0;
    loop {
        if delay > 0 {
            warn!("sleeping for {} seconds", delay);
            sleep(time::Duration::from_secs(delay));
        }

        info!("trying to connect to {}", address);
        let conn = match irc::TcpConn::connect(&address) {
            Ok(conn) => {
                delay = 0;
                conn
            }
            Err(err) => {
                error!("error: {}", err);
                delay += 5;
                continue;
            }
        };

        info!("connected and running");
        run(&config, conn);
        info!("disconnected, respawning");

        delay += 5;
    }
}

fn run(config: &Config, conn: irc::TcpConn) {
    let mut modules: Vec<Arc<Mutex<dyn Module>>> = vec![];
    if let Ok(builtin) = Builtin::create() {
        modules.push(Arc::new(Mutex::new(builtin)));
    }

    // TODO configure 'brain' here
    if let Ok(bard) = Shakespeare::create(vec![
        Box::new(BrainMarkov("http://localhost:7878/markov/next".into())),
        Box::new(BrainMarkov("http://localhost:7879/markov/next".into())),
    ]) {
        modules.push(Arc::new(Mutex::new(bard)))
    }
    if let Ok(poll) = TwitchPoll::create() {
        modules.push(Arc::new(Mutex::new(poll)))
    }
    if let Ok(invest) = Invest::create() {
        modules.push(Arc::new(Mutex::new(invest)))
    }

    let (bot, events) = Bot::create(conn);
    bot.register(&config.twitch.name);

    let (inputs, outputs) = {
        let (mut inputs, mut outputs) = (vec![], vec![]);
        for _ in 0..modules.len() {
            // probably should be bounded
            let (tx, rx) = channel::unbounded();
            inputs.push(tx);
            outputs.push(rx);
        }
        (inputs, outputs)
    };

    let mut pool = Pool::new((modules.len() as u32) + 1);
    pool.scoped(|scope| {
        let (tx, rx) = channel::unbounded();
        for (module, outputs) in modules.into_iter().zip(outputs.iter()) {
            let (sender, outputs) = (tx.clone(), outputs.clone());
            scope.execute(move || module.lock().unwrap().handle(outputs, sender));
        }

        scope.execute(move || bot.process(rx));

        for event in events {
            for input in &inputs {
                input.send(event.clone())
            }
        }
        drop(inputs)
    });
}

fn get_log_level() -> LevelFilter {
    match env::var("SHAKEN_LOG")
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
