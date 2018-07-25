use crate::{bot, config, humanize::*, message, twitch, util};

use chrono::prelude::*; // this should be using serde on the twitch client
use tungstenite as ws;

use std::sync::Arc;

pub struct Builtin;

impl Builtin {
    pub fn new(bot: &bot::Bot, config: &config::Config) -> Self {
        bot.on_raw("PING", |bot, msg| {
            bot.proto().send(&format!("PONG :{}", &msg.data));
        });

        let channels = config.twitch.channels.to_vec();
        bot.on_raw("001", move |bot, _msg| {
            let proto = bot.proto();
            for ch in &channels {
                proto.join(ch)
            }
        });

        bot.on_command("!version", |bot, env| {
            let rev = option_env!("SHAKEN_GIT_REV").unwrap();
            let branch = option_env!("SHAKEN_GIT_BRANCH").unwrap();

            let msg = format!(
                "https://github.com/museun/shaken ({} on '{}' branch)",
                rev, branch
            );

            bot.say(&env, &msg);
        });

        bot.on_command("!shaken", |bot, env| {
            bot.say(
                &env,
                "I try to impersonate The Bard, by being trained on all of his works.",
            );
        });

        bot.on_command("!commands", |bot, env| {
            let commands = bot.get_commands();
            let commands = util::join_with(commands.iter(), " ");
            bot.say(&env, &format!("available commands: {}", commands));
        });

        let twitch = Arc::new(twitch::TwitchClient::new(&config.clone()));

        let next = Arc::clone(&twitch);
        bot.on_command("!viewers", move |bot, env| {
            Self::viewers_command(&next, bot, env)
        });

        let next = Arc::clone(&twitch);
        bot.on_command("!uptime", move |bot, env| {
            Self::uptime_command(&next, bot, env)
        });

        Self {}
    }

    fn viewers_command(
        twitch: &Arc<twitch::TwitchClient>,
        bot: &bot::Bot,
        env: &message::Envelope,
    ) {
        // have to duplicate it because of its scoping. still cleaner this way
        macro_rules! maybe {
            ($e:expr) => {
                if $e {
                    bot.say(&env, "the stream doesn't seem to be live");
                    return;
                }
            };
        }

        // TODO make this configurable
        let streams = twitch.get_streams(&["museun"]);
        maybe!(streams.is_none());

        let streams = streams.unwrap();
        maybe!(streams.is_empty());

        let stream = &streams[0];
        maybe!(stream.live.is_empty() || stream.live == "");

        let viewers = stream.viewer_count.comma_separate();
        bot.say(&env, &format!("viewers: {}", viewers));
    }

    fn get_uptime_from_obs() -> Option<String> {
        // TOOD make this configurable
        if let Ok((mut socket, _r)) =
            ws::connect(::url::Url::parse("ws://localhost:45454").unwrap())
        {
            // this should really be done by serde, but we're only going to send 1 message for now
            let msg = r#"{"request-type":"GetStreamingStatus", "message-id":"0"}"#.to_string();
            socket.write_message(ws::Message::Text(msg)).unwrap();

            // this is awful.
            let msg = socket
                .read_message()
                .map_err(|e| error!("cannot read message from websocket: {}", e))
                .ok()?;
            let msg = msg
                .to_text()
                .map_err(|e| error!("cannot convert message to text: {}", e))
                .ok()?;
            let val = serde_json::from_str::<serde_json::Value>(&msg)
                .map_err(|e| error!("cannot parse json: {}", e))
                .ok()?;

            if val["streaming"].is_boolean() && val["streaming"].as_bool().unwrap() {
                let timecode = val["stream-timecode"].as_str()?;
                return Some(timecode.to_string());
            }
        }
        None
    }

    fn uptime_command(twitch: &Arc<twitch::TwitchClient>, bot: &bot::Bot, env: &message::Envelope) {
        // have to duplicate it because of its scoping. still cleaner this way
        macro_rules! maybe {
            ($e:expr) => {
                if $e {
                    bot.say(&env, "the stream doesn't seem to be live");
                    return;
                }
            };
        }

        let timecode = Self::get_uptime_from_obs();

        // TODO make this configurable
        let streams = twitch.get_streams(&["museun"]);
        maybe!(streams.is_none());

        let streams = streams.unwrap();
        maybe!(streams.is_empty());

        let stream = &streams[0];
        maybe!(stream.live.is_empty() || stream.live == "");

        let start = stream
            .started_at
            .parse::<DateTime<Utc>>()
            .expect("to parse datetime");

        let dur = (Utc::now() - start)
            .to_std()
            .expect("to convert to std duration");

        let mut msg = format!("uptime from twitch: {}", dur.as_readable_time());
        if let Some(timecode) = timecode {
            //01:40:05.671
            let mut map = [("hours", 0), ("minutes", 0), ("seconds", 0)];

            // trim off the .nnn
            for (i, part) in timecode[..timecode.len() - 4]
                .split_terminator(':')
                .take(3)
                .enumerate()
            {
                map[i] = (map[i].0, part.parse::<u64>().unwrap());
            }

            let timecode = ::humanize::format_time_map(&map);

            msg.push_str(&format!(", obs says: {}", &timecode));
        }

        bot.say(&env, &msg);
    }
}
