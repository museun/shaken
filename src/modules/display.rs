use std::fmt::Write;

use {bot, config};

pub struct Display;

impl Display {
    pub fn new(bot: &bot::Bot, _config: &config::Config) -> Self {
        bot.on_passive(|_bot, env| {
            if let Some(nick) = env.get_nick() {
                let display = if let Some(color) = env.tags.get("color") {
                    let (r, g, b) = hex_to_rgb(&color);
                    Color::Turbo(r, g, b).format(&nick)
                } else {
                    nick.into()
                };

                println!("<{}> {}", display, &env.data);
            }
        });

        Self {}
    }
}

enum Color {
    // Blue,
    // Coral,
    // DodgerBlue,
    // SpringGreen,
    // YellowGreen,
    // Green,
    // OrangeRed,
    // Red,
    // GoldenRod,
    // HotPink,
    // CadetBlue,
    // SeaGreen,
    // Chocolate,
    // BlueViolet,
    // Firebrick,
    Turbo(u8, u8, u8),
}

impl From<(u8, u8, u8)> for Color {
    fn from(rgb: (u8, u8, u8)) -> Self {
        let (r, g, b) = rgb;
        Color::Turbo(r, g, b)
    }
}

impl From<&str> for Color {
    fn from(s: &str) -> Self {
        let (r, g, b) = hex_to_rgb(&s);
        Color::Turbo(r, g, b)
    }
}

fn hex_to_rgb(s: &str) -> (u8, u8, u8) {
    let s: String = s.chars().skip(1).collect();
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
    fn format(&self, s: &str) -> String {
        fn wrap(rgb: (u8, u8, u8), s: &str) -> String {
            let (r, g, b) = rgb;
            format!("\x1B[38;2;{};{};{}m{}\x1B[m", r, g, b, s)
        }

        let mut buf = String::new();
        match *self {
            // Color::Blue => write!(buf, "{}", wrap((0, 0, 255), s)),
            // Color::Coral => write!(buf, "{}", wrap((255, 127, 80), s)),
            // Color::DodgerBlue => write!(buf, "{}", wrap((30, 144, 255), s)),
            // Color::SpringGreen => write!(buf, "{}", wrap((0, 255, 127), s)),
            // Color::YellowGreen => write!(buf, "{}", wrap((154, 205, 50), s)),
            // Color::Green => write!(buf, "{}", wrap((0, 255, 0), s)),
            // Color::OrangeRed => write!(buf, "{}", wrap((255, 69, 0), s)),
            // Color::Red => write!(buf, "{}", wrap((255, 0, 0), s)),
            // Color::GoldenRod => write!(buf, "{}", wrap((218, 165, 32), s)),
            // Color::HotPink => write!(buf, "{}", wrap((255, 105, 180), s)),
            // Color::CadetBlue => write!(buf, "{}", wrap((95, 158, 160), s)),
            // Color::SeaGreen => write!(buf, "{}", wrap((46, 139, 87), s)),
            // Color::Chocolate => write!(buf, "{}", wrap((123, 63, 0), s)),
            // Color::BlueViolet => write!(buf, "{}", wrap((75, 0, 130), s)),
            // Color::Firebrick => write!(buf, "{}", wrap((178, 34, 34), s)),
            Color::Turbo(r, g, b) => write!(buf, "{}", wrap((r, g, b), s)).unwrap(),
        };

        buf
    }
}
