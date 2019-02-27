use std::io::{BufReader, Read};
use std::process;
use std::{env, fs};

use brain::*;

struct Options {
    input: Option<String>,
    output: Option<String>,
    address: Option<String>,
    depth: Option<String>,

    action: Action,
}

enum Action {
    Load,
    Train,
}

impl Options {
    fn parse() -> Self {
        let (name, args) = {
            let mut args = env::args();
            (args.next().unwrap(), args.collect::<Vec<_>>())
        };

        let mut opts = getopts::Options::new();
        opts.optflag("h", "help", "prints the usage");
        opts.optflag("t", "train", "enable training mode");
        opts.optflag("l", "load", "enable loading mode");

        opts.optmulti("i", "input", "an input file", "INPUT");
        opts.optopt("o", "output", "the output file", "OUTPUT");
        opts.optopt("d", "depth", "n-gram size", "DEPTH");

        opts.optopt("a", "address", "the address to bind to", "ADDRESS");

        let matches = opts.parse(&args).unwrap();
        if matches.opt_present("h") {
            let brief = format!("Usage: {} -t|-l", name);
            eprintln!("{}", opts.usage(&brief));
            process::exit(1);
        }

        let action = match (matches.opt_present("t"), matches.opt_present("l")) {
            (true, false) => Action::Train,
            (false, true) => Action::Load,
            _ => {
                let brief = format!("Usage: {} -t|-l", name);
                eprintln!("{}", opts.usage(&brief));
                process::exit(1);
            }
        };

        Self {
            input: matches.opt_str("i"),
            output: matches.opt_str("o"),
            address: matches.opt_str("a"),
            depth: matches.opt_str("d"),
            action,
        }
    }

    fn die(&self, msg: impl AsRef<str>) -> ! {
        eprintln!("error: {}", msg.as_ref());
        process::exit(1);
    }
}

fn train(opts: Options) {
    let input = match opts.input {
        Some(input) => input,
        None => opts.die("input file must be specified"),
    };

    let output = opts.output.unwrap_or_else(|| {
        eprintln!("!! assuming you want brain.db");
        "brain.db".into()
    });

    let depth = opts
        .depth
        .map(|d| d.parse::<usize>())
        .unwrap_or_else(|| Ok(5))
        .unwrap();

    brain::train(&input, &output, depth);
}

// TODO don't panic here
fn load(opts: Options) {
    let input = opts.input.unwrap_or_else(|| {
        eprintln!("!! assuming you want brain.db");
        "brain.db".into()
    });

    let file = {
        timeit!("reading {}", input);
        let size = get_file_size(&input).unwrap();
        eprintln!("size: {} KB", size.comma_separate());
        fs::File::open(&input).unwrap()
    };

    let mut buf = Vec::with_capacity(file.metadata().unwrap().len() as usize);
    let mut reader = BufReader::new(file);
    reader.read_to_end(&mut buf).expect("to read file");

    let markov = brain::load(&input, &buf);
    let address = opts
        .address
        .unwrap_or_else(|| "localhost:7878".into())
        .clone();
    let mut server = Server::new(&address, markov);
    server.start();
}

fn main() {
    let opts = Options::parse();

    match opts.action {
        Action::Train => train(opts),
        Action::Load => load(opts),
    }
}
