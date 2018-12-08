use serde_derive::{Deserialize, Serialize};
use std::fmt::{self, Write};

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct RGB(pub u8, pub u8, pub u8);

impl Default for RGB {
    fn default() -> Self {
        RGB(255, 255, 255)
    }
}

impl fmt::Display for RGB {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (r, g, b) = (self.0, self.1, self.2);
        write!(f, "#{:02X}{:02X}{:02X}", r, g, b)
    }
}

impl From<(u8, u8, u8)> for RGB {
    fn from(rgb: (u8, u8, u8)) -> Self {
        RGB(rgb.0, rgb.1, rgb.2)
    }
}

impl From<&str> for RGB {
    fn from(s: &str) -> Self {
        RGB::from(Self::hex_to_rgb(&s))
    }
}

impl From<&String> for RGB {
    fn from(s: &String) -> Self {
        RGB::from(Self::hex_to_rgb(&s))
    }
}

impl From<Option<&String>> for RGB {
    fn from(s: Option<&String>) -> Self {
        match s {
            Some(s) => RGB::from(s),
            None => RGB::from((255, 255, 255)),
        }
    }
}

impl RGB {
    fn hex_to_rgb(s: &str) -> (u8, u8, u8) {
        // should be a #RRGGBB or a RRGGBB
        if (s.len() != 7 && s.len() != 6) || (s.len() == 7 && !s.starts_with('#')) {
            return (255, 255, 255);
        }

        let s = if s.len() == 7 {
            // skip the '#'
            &s[1..]
        } else {
            s
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

    pub fn is_dark(&self) -> bool {
        let (_, _, l) = HSL::from_color(&self).0;
        l < 30.0
    }

    pub fn is_light(&self) -> bool {
        let (_, _, l) = HSL::from_color(&self).0;
        l > 80.0
    }

    pub fn format(&self, s: &str) -> String {
        fn wrap(rgb: (u8, u8, u8), s: &str) -> String {
            let (r, g, b) = rgb;
            format!("\x1B[38;2;{};{};{}m{}\x1B[m", r, g, b, s)
        }

        let mut buf = String::new();
        write!(buf, "{}", wrap((self.0, self.1, self.2), s)).unwrap();
        buf
    }
}

#[derive(PartialEq, Debug)]
pub struct HSL((f64, f64, f64)); // H S L

impl fmt::Display for HSL {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (h, s, l) = self.0;
        write!(f, "{:.2}%, {:.2}%, {:.2}%", h, s, l)
    }
}

impl HSL {
    pub fn from_color(color: &RGB) -> Self {
        #![allow(clippy::many_single_char_names)]
        use std::cmp::{max, min};

        let (r, g, b) = (color.0, color.1, color.2);
        let max = max(max(r, g), b);
        let min = min(min(r, g), b);
        let (r, g, b) = (
            f64::from(r) / 255.0,
            f64::from(g) / 255.0,
            f64::from(b) / 255.0,
        );

        let (min, max) = (f64::from(min) / 255.0, f64::from(max) / 255.0);
        let l = (max + min) / 2.0;
        let delta: f64 = max - min;
        // this checks for grey
        if delta == 0.0 {
            return HSL((0.0, 0.0, ((l * 100.0).round() / 100.0) * 100.0));
        }

        let s = if l < 0.5 {
            delta / (max + min)
        } else {
            delta / (2.0 - max - min)
        };

        let r2 = (((max - r) / 6.0) + (delta / 2.0)) / delta;
        let g2 = (((max - g) / 6.0) + (delta / 2.0)) / delta;
        let b2 = (((max - b) / 6.0) + (delta / 2.0)) / delta;

        let h = match match max {
            x if (x - r).abs() < 0.001 => b2 - g2,
            x if (x - g).abs() < 0.001 => (1.0 / 3.0) + r2 - b2,
            _ => (2.0 / 3.0) + g2 - r2,
        } {
            h if h < 0.0 => h + 1.0,
            h if h > 1.0 => h - 1.0,
            h => h,
        };

        let h = (h * 360.0 * 100.0).round() / 100.0;
        let s = ((s * 100.0).round() / 100.0) * 100.0;
        let l = ((l * 100.0).round() / 100.0) * 100.0;

        HSL((h, s, l))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn hsl() {
        let colors = &[
            (RGB(0, 0, 0), HSL((0.0, 0.0, 0.0)), "black"),
            (RGB(255, 255, 255), HSL((0.0, 0.0, 100.0)), "white"),
            (RGB(255, 0, 0), HSL((0.0, 100.0, 50.0)), "red"),
            (RGB(0, 255, 0), HSL((120.0, 100.0, 50.0)), "lime"),
            (RGB(0, 0, 255), HSL((240.0, 100.0, 50.0)), "blue"),
            (RGB(255, 255, 0), HSL((60.0, 100.0, 50.0)), "yellow"),
            (RGB(0, 255, 255), HSL((180.0, 100.0, 50.0)), "cyan"),
            (RGB(255, 0, 255), HSL((300.0, 100.0, 50.0)), "magneta"),
            (RGB(192, 192, 192), HSL((0.0, 0.0, 75.0)), "silver"),
            (RGB(128, 128, 128), HSL((0.0, 0.0, 50.0)), "gray"),
            (RGB(128, 0, 0), HSL((0.0, 100.0, 25.0)), "maroon"),
            (RGB(128, 128, 0), HSL((60.0, 100.0, 25.0)), "olive"),
            (RGB(0, 128, 0), HSL((120.0, 100.0, 25.0)), "green"),
            (RGB(128, 0, 128), HSL((300.0, 100.0, 25.0)), "purple"),
            (RGB(0, 128, 128), HSL((180.0, 100.0, 25.0)), "teal"),
            (RGB(0, 0, 128), HSL((240.0, 100.0, 25.0)), "navy"),
        ];

        for (rgb, hsl, name) in colors {
            assert_eq!(*hsl, HSL::from_color(&rgb), "{}", name)
        }
    }

    #[test]
    fn to_string() {
        let color = RGB::from("fc0fc0");
        assert_eq!("#FC0FC0".to_string(), color.to_string())
    }
}
