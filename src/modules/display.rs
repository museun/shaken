use crate::{bot, color::Color, config};

pub struct Display;

impl Display {
    pub fn new(bot: &bot::Bot, _config: &config::Config) -> Self {
        bot.set_inspect(move |me, s| {
            // disable @mention display
            if s.starts_with('@') {
                return;
            }

            let display = me.color.format(&me.display);
            println!("<{}> {}", &display, &s)
        });

        bot.on_passive(|_bot, env| {
            // disable !command display
            if env.data.starts_with('!') {
                return;
            }

            if let Some(nick) = env.get_nick() {
                trace!("tags: {:?}", env.tags);
                let display = if let Some(color) = env.tags.get("color") {
                    if let Some(display) = env.tags.get("display-name") {
                        Color::from(color).format(&display)
                    } else {
                        Color::from(color).format(&nick)
                    }
                } else {
                    nick.into()
                };

                println!("<{}> {}", display, &env.data);
            }
        });

        Self {}
    }
}
