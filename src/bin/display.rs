use std::env;
use std::io::prelude::*;
use std::io::BufReader;
use std::net::TcpStream;
use std::str;

use shaken::modules::DisplayMessage;

use termcolor::{Buffer, BufferWriter, Color, ColorChoice, ColorSpec, WriteColor};
use unicode_segmentation::UnicodeSegmentation;

fn main() {
    let (name, mut args) = {
        let mut args = env::args();
        (args.next().unwrap(), args)
    };
    let arg = match args.next() {
        Some(arg) => arg,
        None => die(format!("usage: {} addr:port", name)),
    };

    if let Err(err) = Client::connect(&arg) {
        die(format!("client error: {:?}", err))
    }
}

fn die(s: impl AsRef<str>) -> ! {
    eprintln!("{}", s.as_ref());
    std::process::exit(1)
}

struct Client;
impl Client {
    pub fn connect(addr: &str) -> Result<(), Error> {
        let conn = TcpStream::connect(addr).map_err(|_e| Error::CannotConnect)?;
        let mut reader = BufReader::new(conn).lines();

        // TODO from the env
        let use_colors = true;

        let buffer = BufferWriter::stdout(if use_colors {
            ColorChoice::Auto
        } else {
            ColorChoice::Never
        });

        while let Some(Ok(line)) = reader.next() {
            let msg = serde_json::from_str::<DisplayMessage>(&line).expect("valid json");
            let mut buf = buffer.buffer();
            let mut buf = Self::add_name(&msg, &mut buf);
            let lines = Self::split_lines(&msg);
            let buf = Self::print_lines(&lines, &mut buf);
            buffer.print(&buf).expect("print");
        }
        Ok(())
    }

    fn add_name<'a>(msg: &DisplayMessage, buf: &'a mut Buffer) -> &'a mut Buffer {
        let mut spec = ColorSpec::new();
        let (r, g, b) = (msg.color.0, msg.color.1, msg.color.2);
        spec.set_fg(Some(Color::Rgb(r, g, b)));
        buf.set_color(&spec).expect("set color");

        let name = truncate(msg.name.clone());
        write!(buf, "{:>offset$}: ", name, offset = NAME_MAX + 1);
        buf.reset().expect("reset");
        buf
    }

    fn split_lines(msg: &DisplayMessage) -> Vec<String> {
        let mut lines = vec![];
        let mut line = String::new();
        for s in msg.data.split_word_bounds() {
            if s.len() > LINE_MAX {
                let mut tmp = String::new();
                for c in s.chars() {
                    if line.len() == LINE_MAX {
                        lines.push(line.clone());
                        line.clear();
                    }
                    if tmp.len() == LINE_MAX {
                        line.push_str(&tmp);
                        tmp.clear();
                    }
                    tmp.push(c);
                }

                if line.len() == LINE_MAX {
                    lines.push(line.clone());
                    line.clear();
                }
                if !tmp.is_empty() {
                    line.push_str(&tmp)
                }
                continue;
            }
            if line.len() + s.len() > LINE_MAX {
                lines.push(line.clone());
                line.clear();
            }
            line.push_str(&s);
        }
        if !line.is_empty() {
            lines.push(line);
        }
        lines
    }

    fn print_lines<'a>(lines: &[String], buf: &'a mut Buffer) -> &'a mut Buffer {
        let pad: String = std::iter::repeat(" ")
            .take(NAME_MAX + 2)
            .collect::<String>(); // probably should be passed in as an arg

        for (i, s) in lines.iter().map(|s| s.trim_left()).enumerate() {
            if i == 0 {
                write!(buf, "{}", s);
            } else {
                write!(buf, "{}{}{}", LEFT_FRINGE, pad, s);
            }
            if lines.len() == 1 {
                writeln!(buf);
                continue;
            }
            if i < lines.len() - 1 {
                let len = LINE_MAX.saturating_sub(s.len()) + 3;
                writeln!(buf, "{: >width$}", RIGHT_FRINGE, width = len);
            } else {
                writeln!(buf);
            }
        }
        buf
    }
}

const LINE_MAX: usize = 60;
const NAME_MAX: usize = 10;
const LEFT_FRINGE: char = '\u{1F4A9}';
const RIGHT_FRINGE: char = '\u{1F6BD}';

//\u{2502}  // thin
//\u{257F}  // split

// \u{2937}
// \u{2936}

fn truncate(mut s: String) -> String {
    if s.len() <= NAME_MAX {
        return s;
    }

    s.truncate(NAME_MAX);
    s.insert(NAME_MAX, '…');
    s
}

#[derive(Debug, PartialEq)]
enum Error {
    CannotConnect,
}

// the more I look at this, the more I'll decide on what to do with it
// I like the match for match, but..
// match msg.badges.iter().fold((false, false), |(mut b, mut m), c| {
//     match c {
//         irc::Badge::Broadcaster => b = true,
//         irc::Badge::Moderator => m = true,
//         _ => {}
//     };
//     (b, m)
// }) {
//     (true, _) => {
//         write!(buf, "\u{1F3A5}  ");
//     }
//     (_, true) => {
//         write!(buf, "\u{1F6E1}  ");
//     }
//     _ => {}
// };
