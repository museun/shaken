use std::io::{prelude::*, BufReader};

use log::*;
use simplelog::{Config as LogConfig, TermLogger};

mod host;
mod markov;

use shaken::util::{self, CommaSeparated};

#[macro_export]
macro_rules! abort {
    ($f:expr, $($args:expr),* $(,)?) => {{
        let msg = format!($f, $($args),*);
        error!("{}", msg);
        if cfg!(test) {
            panic!("{}", msg);
        }
        ::std::process::exit(1);
    }};
    ($e:expr) => {{
        error!("{}", $e);
        if cfg!(test) {
            panic!("{}", $e);
        }
        ::std::process::exit(1);
    }};
}

fn train<R, W>(mut input: R, len: usize, mut output: W, depth: usize)
where
    R: Read,
    W: Write,
{
    let mut buf = String::with_capacity(len);
    input.read_to_string(&mut buf).unwrap();

    let mut markov = markov::Markov::with_depth(depth);
    info!("training with depth: {}", depth);

    markov.train_text(&buf);

    let data = bincode::serialize(&markov).unwrap();
    output.write_all(&data).unwrap();
}

fn load<R>(file: R, len: usize, addr: &str)
where
    R: Read,
{
    let mut buf = Vec::with_capacity(len);
    let mut reader = BufReader::new(file);
    reader.read_to_end(&mut buf).expect("to read file");
    let markov = bincode::deserialize(&buf).expect("deserialize file");
    host::Server::new(&addr, markov).start();
}

fn main() {
    TermLogger::init(
        util::get_log_level("BRAIN_LOG"),
        LogConfig::default(), // some config
    )
    .expect("initialize logger");

    let mut args = std::env::args();
    let name = args.next().unwrap();

    macro_rules! next {
        () => {
            args.next().as_ref().map(String::as_str)
        };
    }

    let data_dir = data_dir();

    match (next!(), next!(), next!(), next!()) {
        (Some("train"), Some(input), Some(output), depth) => {
            let depth = depth
                .and_then(|d| d.parse::<usize>().ok())
                .unwrap_or_else(|| 5);

            let output = data_dir.join(output);

            match (std::fs::File::open(input), std::fs::File::create(&output)) {
                (Err(err), ..) => abort!("invalid input file at {}. {}", input, err),
                (.., Err(err)) => abort!(
                    "invalid output file at {}. {}",
                    output.to_string_lossy(),
                    err
                ),
                (Ok(in_file), Ok(out_file)) => {
                    let size = util::get_file_size(&input).unwrap();
                    info!("size: {} KB", size.commas());
                    train(in_file, size as usize, out_file, depth);
                    let size = util::get_file_size(&output).unwrap();
                    info!("size: {} KB", size.commas());
                }
            }
        }

        (Some("load"), Some(input), addr, ..) => {
            let input = data_dir.join(input);

            let file = match std::fs::File::open(&input) {
                Ok(file) => file,
                Err(err) => abort!("invalid input file at {}. {}", input.to_string_lossy(), err),
            };

            let size = util::get_file_size(&input).unwrap();
            info!("size: {} KB", size.commas());

            let addr = addr.unwrap_or_else(|| "localhost:7878");

            let len = file.metadata().unwrap().len() as usize;
            load(file, len, addr)
        }

        _ => {
            let path: std::path::PathBuf = name.into();
            let name = path.file_stem().unwrap().to_string_lossy();
            error!("usage: {} train <input> <output> <depth?>", &name);
            error!("       {} load <input> <address?>", name);
            std::process::exit(1);
        }
    }
}

fn data_dir() -> std::path::PathBuf {
    let dir = directories::ProjectDirs::from("com.github", "museun", "brain").unwrap();
    std::fs::create_dir_all(dir.data_dir()).expect("must be able to create project dirs");
    dir.data_dir().to_path_buf()
}
