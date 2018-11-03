use std::io::prelude::*;
use std::io::BufReader;
use std::net::TcpStream;
use std::{env, str};

use shaken::modules::DisplayMessage;

use termcolor::{BufferWriter, Color, ColorChoice, ColorSpec, WriteColor};
use unicode_segmentation::UnicodeSegmentation;

fn main() {
    let args = env::args().collect::<Vec<_>>();
    let opts = Opts::parse(&args);
    if let Err(err) = Client::connect(&opts) {
        die(format!("client error: {:?}", err))
    }
}

fn die(s: impl AsRef<str>) -> ! {
    eprintln!("{}", s.as_ref());
    std::process::exit(1)
}

struct Buffer<'a> {
    writer: &'a termcolor::BufferWriter,
    buf: termcolor::Buffer,
    opts: &'a Opts,
    pad: String,
    msg: &'a DisplayMessage,
    lines: Vec<String>,
}

impl<'a> Buffer<'a> {
    pub fn new(
        buffer: &'a termcolor::BufferWriter,
        opts: &'a Opts,
        msg: &'a DisplayMessage,
    ) -> Self {
        let pad: String = std::iter::repeat(" ")
            .take(opts.name_max + 2)
            .collect::<String>();

        Self {
            writer: buffer,
            buf: buffer.buffer(),
            pad,
            opts,
            msg,
            lines: vec![], // this allocates 7..
        }
    }

    pub fn print(mut self) {
        self.add_name(self.msg.is_action);
        self.split_lines();
        self.write_lines();
        self.writer.print(&self.buf).expect("print");
    }

    fn add_name(&mut self, action: bool) {
        let mut name = self.msg.name.clone();
        let name = self.truncate(&mut name);
        let pad = self.opts.name_max.saturating_sub(name.len()) + 1;

        if action {
            write!(self.buf, "{:>offset$}", "*", offset = pad);
        } else {
            write!(self.buf, "{:>offset$}", " ", offset = pad);
        }

        let mut spec = ColorSpec::new();
        let (r, g, b) = (self.msg.color.0, self.msg.color.1, self.msg.color.2);
        spec.set_fg(Some(Color::Rgb(r, g, b)));
        self.buf.set_color(&spec).expect("set color");
        write!(self.buf, "{}", name);
        self.buf.reset().expect("reset");

        if action {
            write!(self.buf, " ");
        } else {
            write!(self.buf, ": ");
        }
    }

    fn write_lines(&mut self) {
        for (i, s) in self.lines.iter().map(|s| s.trim_left()).enumerate() {
            if i == 0 {
                write!(self.buf, "{}", s);
            } else {
                write!(self.buf, "{}{}{}", self.opts.left, self.pad, s);
            }
            if self.lines.len() == 1 {
                writeln!(self.buf);
                continue;
            }
            if i < self.lines.len() - 1 {
                let len = self.opts.line_max.saturating_sub(s.len());
                writeln!(self.buf, "{: >width$}", self.opts.right, width = len);
            } else {
                writeln!(self.buf);
            }
        }
    }

    fn split_lines(&mut self) {
        let max = self.opts.line_max;

        let mut lines = vec![];
        let mut line = String::new();
        for s in self.msg.data.split_word_bounds() {
            if s.len() >= max {
                let mut tmp = String::new();
                for c in s.chars() {
                    if line.len() == max {
                        lines.push(line.clone());
                        line.clear();
                    }
                    if tmp.len() == max {
                        line.push_str(&tmp);
                        tmp.clear();
                    }
                    tmp.push(c);
                }

                if line.len() == max {
                    lines.push(line.clone());
                    line.clear();
                }
                if !tmp.is_empty() {
                    line.push_str(&tmp)
                }
                continue;
            }
            if line.len() + s.len() >= max {
                lines.push(line.clone());
                line.clear();
            }
            line.push_str(&s);
        }
        if !line.is_empty() {
            lines.push(line);
        }

        std::mem::replace(&mut self.lines, lines);
    }

    fn truncate<'b>(&self, name: &'b mut String) -> &'b String {
        let max = self.opts.name_max;
        if name.len() <= max {
            return name;
        }
        name.truncate(max);
        name.insert(max, '…');
        name
    }
}

struct Client;
impl Client {
    pub fn connect(opts: &Opts) -> Result<(), Error> {
        let conn = TcpStream::connect(&opts.addr).map_err(|_e| Error::CannotConnect)?;
        let mut reader = BufReader::new(conn).lines();

        let colors = if env::var("NO_COLOR").is_err() {
            ColorChoice::Auto
        } else {
            ColorChoice::Never
        };
        let buffer = BufferWriter::stdout(colors);

        while let Some(Ok(line)) = reader.next() {
            let msg = serde_json::from_str::<DisplayMessage>(&line).expect("valid json");
            Buffer::new(&buffer, &opts, &msg).print();
        }
        Ok(())
    }
}

#[derive(Debug, PartialEq)]
enum Error {
    CannotConnect,
}

struct Opts {
    left: char,
    right: char,
    addr: String,
    line_max: usize,
    name_max: usize,
}

// this is ugly
impl Opts {
    pub fn parse(args: &[String]) -> Self {
        use getopts::Options;
        let mut opts = Options::new();
        opts.optflag("h", "help", "prints help information");
        opts.optopt(
            "a",
            "addr",
            "the address to connect to. defaults to 'localhost:51001`",
            "ip:port",
        );
        opts.optopt(
            "l",
            "left",
            "left fringe character. defaults to '\u{2937}'",
            "char",
        );
        opts.optopt(
            "r",
            "right",
            "right fringe character. defaults to '\u{2936}'",
            "char",
        );
        opts.optopt(
            "n",
            "name",
            "width of names before truncation. defaults to '10'",
            "size",
        );

        // TODO or default terminal size

        let matches = match opts.parse(&args[1..]) {
            Ok(opts) => opts,
            Err(err) => die(format!("couldn't parse args: {}", err)),
        };

        if matches.opt_present("h") {
            print!("{}", opts.usage(&format!("usage: {}", &args[0])));
            std::process::exit(0);
        }

        let addr = matches
            .opt_get_default("a", "localhost:51001".into())
            .expect("address arg");

        let name_max = matches.opt_get_default("n", 10usize).expect("name_max arg");

        let left = matches
            .opt_get_default("l", '\u{2937}')
            .expect("left fringe arg");

        let right = matches
            .opt_get_default("r", '\u{2936}')
            .expect("right fringe arg");

        let line_max = term_size::dimensions()
            .and_then(|(w, _)| Some(w - 2))
            .unwrap_or_else(|| 60 - 2)
            - (name_max); // fringe+space

        Opts {
            left,
            right,
            addr,
            line_max,
            name_max,
        }
    }
}
