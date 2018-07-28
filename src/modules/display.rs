use crate::{bot, color::Color, config, message::Envelope};

use parking_lot::Mutex;
use std::collections::HashMap;
use std::sync::Arc;

pub struct Display {
    colors: Mutex<HashMap<String, Color>>,
}

impl Display {
    pub fn new(bot: &bot::Bot, _config: &config::Config) -> Arc<Self> {
        let colors = {
            ::std::fs::File::open("colors.json")
                .map_err(|_| None)
                .and_then(|f| {
                    serde_json::from_reader(&f).map_err(|e| {
                        error!("cannot load colors: {}", e);
                        None
                    })
                })
                .or_else::<HashMap<String, Color>, _>(|_: Option<()>| Ok(HashMap::new()))
                .unwrap()
        };

        let this = Arc::new(Self {
            colors: Mutex::new(colors),
        });

        bot.set_inspect(move |me, s| {
            // disable @mention display
            if s.starts_with('@') {
                return;
            }

            let display = me.color.format(&me.display);
            println!("<{}> {}", &display, &s)
        });

        let next = Arc::clone(&this);
        bot.on_command("!color", move |bot, env| {
            let badcolors = &[
                Color::from((0, 0, 0)), // black
            ];

            if let Some(id) = env.get_id() {
                if let Some(part) = env.data.split_whitespace().collect::<Vec<_>>().get(0) {
                    let color = Color::from(*part);

                    for bad in badcolors {
                        if color == *bad {
                            bot.reply(&env, "don't use that color");
                            return;
                        }
                    }
                    {
                        let mut colors = next.colors.lock();
                        colors.insert(id.to_string(), color);
                    }
                    {
                        let colors = next.colors.lock();
                        if let Ok(f) = ::std::fs::File::create("colors.json") {
                            let _ = serde_json::to_writer(&f, &*colors).map_err(|e| {
                                error!("cannot save colors: {}", e);
                            });
                        }
                    }
                }
            }
        });

        // --

        let next = Arc::clone(&this);
        bot.on_passive(move |_bot, env| {
            fn get_color_for(map: &HashMap<String, Color>, env: &'a Envelope) -> Option<Color> {
                map.get(env.get_id()?).cloned().or_else(|| {
                    env.tags
                        .get("color")
                        .and_then(|s| Some(Color::from(s)))
                        .or_else(|| Some(Color::from((255, 255, 255))))
                })
            }

            if let Some(nick) = env.get_nick() {
                trace!("tags: {:?}", env.tags);

                let color = {
                    let map = next.colors.lock();
                    get_color_for(&map, &env)
                }.unwrap();

                let display = env
                    .tags
                    .get("display-name")
                    .and_then(|s| Some(s.as_ref()))
                    .or_else(|| Some(nick))
                    .unwrap();

                if env.data.starts_with('!') {
                    return;
                }
                println!("<{}> {}", color.format(&display), &env.data);
            }
        });

        this
    }
}
