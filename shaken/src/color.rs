use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Copy, Clone, PartialEq, Deserialize, Serialize)]
pub struct RGB(pub u8, pub u8, pub u8);

impl Default for RGB {
    fn default() -> Self {
        RGB(255, 255, 255)
    }
}

impl fmt::Display for RGB {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let RGB(r, g, b) = self;
        write!(f, "#{:02X}{:02X}{:02X}", r, g, b)
    }
}

impl From<(u8, u8, u8)> for RGB {
    fn from((r, g, b): (u8, u8, u8)) -> Self {
        RGB(r, g, b)
    }
}

impl From<&str> for RGB {
    fn from(s: &str) -> Self {
        let s = s.trim();
        let s = match (s.chars().next(), s.len()) {
            (Some('#'), 7) => &s[1..],
            (.., 6) => s,
            _ => return Self::default(),
        };

        u32::from_str_radix(&s, 16)
            .and_then(|s| {
                Ok(RGB(
                    ((s >> 16) & 0xFF) as u8,
                    ((s >> 8) & 0xFF) as u8,
                    (s & 0xFF) as u8,
                ))
            })
            .unwrap_or_default()
    }
}

impl From<&String> for RGB {
    fn from(s: &String) -> Self {
        s.as_str().into()
    }
}

impl From<Option<&String>> for RGB {
    fn from(s: Option<&String>) -> Self {
        s.map(|s| s.into()).unwrap_or_default()
    }
}

impl RGB {
    pub fn is_dark(self) -> bool {
        let HSL(_, _, l) = self.into();
        l < 30.0
    }

    pub fn is_light(self) -> bool {
        let HSL(_, _, l) = self.into();
        l > 80.0
    }
}

#[derive(PartialEq, Copy, Clone, Debug)]
pub struct HSL(pub f64, pub f64, pub f64); // H S L

impl fmt::Display for HSL {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let HSL(h, s, l) = self;
        write!(f, "{:.2}%, {:.2}%, {:.2}%", h, s, l)
    }
}

impl From<RGB> for HSL {
    fn from(RGB(r, g, b): RGB) -> Self {
        #![allow(clippy::unknown_clippy_lints, clippy::many_single_char_names)]
        use std::cmp::{max, min};

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
            return HSL(0.0, 0.0, ((l * 100.0).round() / 100.0) * 100.0);
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

        HSL(h, s, l)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn hsl() {
        let colors = &[
            (RGB(0, 0, 0), HSL(0.0, 0.0, 0.0), "black"),
            (RGB(255, 255, 255), HSL(0.0, 0.0, 100.0), "white"),
            (RGB(255, 0, 0), HSL(0.0, 100.0, 50.0), "red"),
            (RGB(0, 255, 0), HSL(120.0, 100.0, 50.0), "lime"),
            (RGB(0, 0, 255), HSL(240.0, 100.0, 50.0), "blue"),
            (RGB(255, 255, 0), HSL(60.0, 100.0, 50.0), "yellow"),
            (RGB(0, 255, 255), HSL(180.0, 100.0, 50.0), "cyan"),
            (RGB(255, 0, 255), HSL(300.0, 100.0, 50.0), "magneta"),
            (RGB(192, 192, 192), HSL(0.0, 0.0, 75.0), "silver"),
            (RGB(128, 128, 128), HSL(0.0, 0.0, 50.0), "gray"),
            (RGB(128, 0, 0), HSL(0.0, 100.0, 25.0), "maroon"),
            (RGB(128, 128, 0), HSL(60.0, 100.0, 25.0), "olive"),
            (RGB(0, 128, 0), HSL(120.0, 100.0, 25.0), "green"),
            (RGB(128, 0, 128), HSL(300.0, 100.0, 25.0), "purple"),
            (RGB(0, 128, 128), HSL(180.0, 100.0, 25.0), "teal"),
            (RGB(0, 0, 128), HSL(240.0, 100.0, 25.0), "navy"),
        ];

        for &(rgb, hsl, name) in colors.iter() {
            assert_eq!(hsl, HSL::from(rgb), "{}", name)
        }
    }

    #[test]
    fn to_string() {
        let color = RGB::from("fc0fc0");
        assert_eq!("#FC0FC0".to_string(), color.to_string())
    }
}
