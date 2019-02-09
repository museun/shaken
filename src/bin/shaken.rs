use crossbeam_channel as channel;
use hashbrown::HashMap;
use log::{debug, error, info, warn};
use scoped_threadpool::Pool;
use simplelog::{Config as LogConfig, LevelFilter, TermLogger};
use termcolor::{BufferWriter, Color, ColorChoice, ColorSpec, WriteColor};

use std::io::Write;
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

    parse_args();

    let config = Config::load();
    let (modules, disabled) = create_modules(&config);

    let printer = Printer::new(&disabled);
    printer.modules();
    if !disabled.is_empty() {
        printer.disabled_modules();
    }
    printer.system_commands();
    if !disabled.contains(&"Builtin") {
        printer.user_commands();
    }

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
        run(&config, conn, &modules);
        info!("disconnected, respawning");

        delay += 5;
    }
}

fn parse_args() {
    if let "config" = std::env::args().nth(1).unwrap_or_default().as_str() {
        let file = config::get_config_file().unwrap_or_else(|| {
            error!("system does not have a standard directory for configuration files. aborting");
            std::process::exit(1)
        });

        if std::fs::metadata(&file).is_ok() {
            warn!(
                "configuration file already exists at: {}",
                file.to_string_lossy()
            );
            warn!("delete it and rerun command to generate a default configuration");
            std::process::exit(1)
        }

        info!(
            "creating a default configuration in: {}",
            file.to_string_lossy()
        );
        Config::default().save();
        std::process::exit(0)
    }
}

type LoadedModule = Arc<Mutex<dyn Module>>;
fn create_modules(config: &Config) -> (Vec<LoadedModule>, Vec<&'static str>) {
    let mut modules: Vec<Arc<Mutex<dyn Module>>> = vec![];
    let mut disabled = vec![];

    macro_rules! create {
        ($e:path) => {{
            let name = stringify!($e).split("::").next().unwrap();
            if config.enabled.iter().any(|m| m == name) {
                if let Ok(m) = $e() {
                    info!("loaded module: {}", name);
                    modules.push(Arc::new(Mutex::new(m)))
                }
            } else {
                disabled.push(name);
            }
        }};
        ($e:path, $($f:expr),+) => {{
            let name = stringify!($e).split("::").next().unwrap();
            if config.enabled.iter().any(|m| m == name) {
                if let Ok(m) = $e($($f)*) {
                    info!("loaded module: {}", name);
                    modules.push(Arc::new(Mutex::new(m)))
                }
            } else {
                disabled.push(name)
            }
        }};
    }

    create!(Builtin::create);
    create!(CurrentSong::create);
    create!(TwitchPoll::create);
    create!(Invest::create);
    create!(RustStuff::create);

    let brains = config
        .shakespeare
        .brains
        .to_vec()
        .into_iter()
        .inspect(|brain| info!("creating BrainMarkov for: {}", brain))
        .map(|url| Box::new(BrainMarkov(url.into())) as Box<dyn Markov + 'static>)
        .collect::<Vec<_>>();

    if !brains.is_empty() {
        create!(Shakespeare::create, brains);
    }

    (modules, disabled)
}

fn run(config: &Config, conn: irc::TcpConn, modules: &[LoadedModule]) {
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
        for (module, outputs) in modules.iter().zip(outputs) {
            let (sender, outputs) = (tx.clone(), outputs.clone());
            scope.execute(move || module.lock().unwrap().handle(outputs, sender));
        }

        scope.execute(move || bot.process(rx));

        for event in events {
            for input in &inputs {
                let _ = input.send(event.clone());
            }
        }
        drop(inputs)
    });
}

macro_rules! colorln {
    ($buffer:expr, $color:expr, $($args:expr),*) => {
        $buffer.set_color(&$color).unwrap();
        writeln!(&mut $buffer, "{}", format_args!($($args),*)).unwrap()
    };
}
macro_rules! color {
    ($buffer:expr, $color:expr, $($args:expr),*) => {
        $buffer.set_color(&$color).unwrap();
        write!(&mut $buffer, "{}", format_args!($($args),*)).unwrap()
    };
}

struct Printer<'a> {
    disabled: &'a [&'static str],
    green: ColorSpec,
    yellow: ColorSpec,
    magenta: ColorSpec,
    white: ColorSpec,
    blue: ColorSpec,
    writer: BufferWriter,
}

impl<'a> Printer<'a> {
    pub fn new(disabled: &'a [&'static str]) -> Self {
        let green = ColorSpec::new().set_fg(Some(Color::Green)).clone();
        let yellow = ColorSpec::new().set_fg(Some(Color::Yellow)).clone();
        let magenta = ColorSpec::new().set_fg(Some(Color::Magenta)).clone();
        let white = ColorSpec::new().set_fg(Some(Color::White)).clone();
        let blue = ColorSpec::new().set_fg(Some(Color::Blue)).clone();

        let writer = if std::env::var("NO_COLOR").is_err() {
            BufferWriter::stdout(ColorChoice::Auto)
        } else {
            BufferWriter::stdout(ColorChoice::Never)
        };

        Self {
            disabled,
            green,
            yellow,
            magenta,
            white,
            blue,
            writer,
        }
    }

    fn modules(&self) {
        let mut buffer = self.writer.buffer();
        colorln!(&mut buffer, self.magenta, "loaded modules:");
        color!(&mut buffer, self.green, "- ");
        for (i, m) in MODULES
            .iter()
            .filter(|s| !self.disabled.contains(&s))
            .enumerate()
        {
            color!(&mut buffer, self.white, "{}", m);
            if i < MODULES.len().saturating_sub(1) {
                color!(&mut buffer, self.green, ", ");
            }
        }
        writeln!(&mut buffer).unwrap();
        self.writer.print(&buffer).unwrap();
        buffer.clear();
    }

    fn disabled_modules(&self) {
        let mut buffer = self.writer.buffer();
        colorln!(&mut buffer, self.magenta, "disabled modules:");
        color!(&mut buffer, self.green, "- ");
        for (i, m) in self.disabled.iter().enumerate() {
            color!(&mut buffer, self.white, "{}", m);
            if i < self.disabled.len().saturating_sub(1) {
                color!(&mut buffer, self.green, ", ");
            }
        }
        writeln!(&mut buffer).unwrap();
        self.writer.print(&buffer).unwrap();
        buffer.clear();
    }

    fn system_commands(&self) {
        let mut buffer = self.writer.buffer();
        let mut map = HashMap::new();
        for (k, v) in Registry::commands()
            .into_iter()
            .map(|cmd| (cmd.namespace().to_string(), cmd.name().to_string()))
        {
            map.entry(k).or_insert(vec![]).push(v);
        }

        colorln!(&mut buffer, self.magenta, "list of system commands:");
        buffer.set_color(&ColorSpec::new()).unwrap();
        for (k, list) in map
            .into_iter()
            .filter(|(k, _)| !self.disabled.contains(&(k.as_ref())))
        {
            colorln!(&mut buffer, self.yellow, "{}", k);
            color!(&mut buffer, self.green, "- ");

            for (i, v) in list.iter().enumerate() {
                color!(&mut buffer, self.white, "{}", v);
                if i < list.len().saturating_sub(1) {
                    color!(&mut buffer, self.green, ", ");
                }
            }
            writeln!(&mut buffer).unwrap()
        }
        self.writer.print(&buffer).unwrap();
        buffer.clear();
    }

    fn user_commands(&self) {
        let mut buffer = self.writer.buffer();
        let mut map = HashMap::new();
        for cmd in Builtin::fetch_command_names() {
            if let Some(UserCommand {
                creator, command, ..
            }) = Builtin::try_get_command(&cmd)
            {
                map.entry(creator).or_insert(vec![]).push(command);
            }
        }

        let conn = database::get_connection();
        colorln!(&mut buffer, self.magenta, "list of user commands:");
        for (k, n, list) in map.into_iter().filter_map(|(k, v)| {
            UserStore::get_user_by_id(&conn, k)
                .or_else(|| {
                    warn!("unknown user with id: {}", k);
                    warn!("they own {} commands: {}", v.len(), v.join(", "));
                    None
                })
                .and_then(|User { display, .. }| Some((k, display, v)))
        }) {
            color!(&mut buffer, self.yellow, "commands from ");
            color!(&mut buffer, self.green, "{}", n);
            color!(&mut buffer, self.yellow, " (");
            color!(&mut buffer, self.blue, "{}", k);
            colorln!(&mut buffer, self.yellow, ")");

            color!(&mut buffer, self.green, "- ");
            for (i, v) in list.iter().enumerate() {
                color!(&mut buffer, self.white, "{}", v);
                if i < list.len().saturating_sub(1) {
                    color!(&mut buffer, self.green, ", ");
                }
            }
            writeln!(&mut buffer).unwrap()
        }
        self.writer.print(&buffer).unwrap();
    }
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
