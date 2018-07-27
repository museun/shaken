#![allow(dead_code)]
use std::fmt::Write;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Color {
    Turbo(u8, u8, u8),
}

impl From<(u8, u8, u8)> for Color {
    fn from(rgb: (u8, u8, u8)) -> Self {
        let (r, g, b) = rgb;
        trace!("got color: {},{},{}", r, g, b);
        Color::Turbo(r, g, b)
    }
}

impl From<&str> for Color {
    fn from(s: &str) -> Self {
        let (r, g, b) = hex_to_rgb(&s);
        Color::from((r, g, b))
    }
}

impl From<&String> for Color {
    fn from(s: &String) -> Self {
        let (r, g, b) = hex_to_rgb(&s);
        Color::from((r, g, b))
    }
}

impl From<Option<&String>> for Color {
    fn from(s: Option<&String>) -> Self {
        match s {
            Some(s) => Color::from(s),
            None => Color::from((255, 255, 255)),
        }
    }
}

fn hex_to_rgb(s: &str) -> (u8, u8, u8) {
    if (s.len() != 7 && s.len() != 6) || (s.len() == 7 && !s.starts_with('#')) {
        return (255, 255, 255);
    }

    let s: String = if s.len() == 7 {
        // skip the '#'
        s.chars().skip(1).collect()
    } else {
        s.chars().collect()
    };

    if let Ok(s) = u32::from_str_radix(&s, 16) {
        let r = ((s >> 16) & 0xFF) as u8;
        let g = ((s >> 8) & 0xFF) as u8;
        let b = (s & 0xFF) as u8;
        (r, g, b)
    } else {
        (255, 255, 255)
    }
}

impl Color {
    pub fn format(&self, s: &str) -> String {
        fn wrap(rgb: (u8, u8, u8), s: &str) -> String {
            let (r, g, b) = rgb;
            format!("\x1B[38;2;{};{};{}m{}\x1B[m", r, g, b, s)
        }

        let mut buf = String::new();
        match *self {
            Color::Turbo(r, g, b) => write!(buf, "{}", wrap((r, g, b), s)).unwrap(),
        };

        buf
    }
}
