use crate::{bot::*, config, humanize::*, message, twitch, util};

use chrono::prelude::*;
use std::sync::Arc;

pub struct Builtin {
    twitch: twitch::TwitchClient,
}

impl Builtin {
    pub fn new(bot: &Bot, _config: &config::Config) -> Arc<Self> {
        let this = Arc::new(Self {
            twitch: twitch::TwitchClient::new(),
        });

        let next = Arc::clone(&this);
        bot.on_raw("PING", move |bot, msg| {
            next.ping_raw(bot, msg);
            None
        });

        let next = Arc::clone(&this);
        bot.on_raw("001", move |bot, msg| {
            next.autojoin_raw(bot, msg);
            None
        });

        let next = Arc::clone(&this);
        bot.on_command("!version", move |bot, env| {
            next.version_command(bot, env);
            None
        });

        let next = Arc::clone(&this);
        bot.on_command("!shaken", move |bot, env| {
            next.shaken_command(bot, env);
            None
        });

        let next = Arc::clone(&this);
        bot.on_command("!commands", move |bot, env| {
            next.commands_command(bot, env);
            None
        });

        let next = Arc::clone(&this);
        bot.on_command("!viewers", move |bot, env| {
            next.viewers_command(bot, env);
            None
        });

        let next = Arc::clone(&this);
        bot.on_command("!uptime", move |bot, env| {
            next.uptime_command(bot, env);
            None
        });

        this
    }

    fn autojoin_raw(&self, bot: &Bot, _msg: &message::Message) {
        bot.join(&format!("#{}", &bot.channel));
    }

    fn ping_raw(&self, bot: &Bot, msg: &message::Message) {
        bot.send(&format!("PONG :{}", &msg.data))
    }

    fn version_command(&self, bot: &Bot, env: &message::Envelope) {
        let rev = option_env!("SHAKEN_GIT_REV").unwrap();
        let branch = option_env!("SHAKEN_GIT_BRANCH").unwrap();

        let msg = format!(
            "https://github.com/museun/shaken ({} on '{}' branch)",
            rev, branch
        );

        bot.say(&env, &msg);
    }

    fn shaken_command(&self, bot: &Bot, env: &message::Envelope) {
        bot.say(
            &env,
            "I try to impersonate The Bard, by being trained on all of his works.",
        );
    }

    fn commands_command(&self, bot: &Bot, env: &message::Envelope) {
        let commands = bot.get_commands();
        let commands = util::join_with(commands.iter(), " ");
        bot.say(&env, &format!("available commands: {}", commands));
    }

    fn viewers_command(&self, bot: &Bot, env: &message::Envelope) {
        // have to duplicate it because of its scoping. still cleaner this way
        macro_rules! maybe {
            ($e:expr) => {
                if $e {
                    bot.say(&env, "the stream doesn't seem to be live");
                    return;
                }
            };
        }

        let streams = self.twitch.get_streams(&[&bot.channel]);
        maybe!(streams.is_none());

        let streams = streams.unwrap();
        maybe!(streams.is_empty());

        let stream = &streams[0];
        maybe!(stream.live.is_empty() || stream.live == "");

        let viewers = stream.viewer_count.comma_separate();
        bot.say(&env, &format!("viewers: {}", viewers));
    }

    fn uptime_command(&self, bot: &Bot, env: &message::Envelope) {
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

        let streams = self.twitch.get_streams(&[&bot.channel]);
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

            let timecode = crate::humanize::format_time_map(&map);
            msg.push_str(&format!(", obs says: {}", &timecode));
        }

        bot.say(&env, &msg);
    }

    fn get_uptime_from_obs() -> Option<String> {
        fn get_inner(tx: &crossbeam_channel::Sender<String>) -> Option<()> {
            use tungstenite as ws;

            let conn = ws::connect(::url::Url::parse("ws://localhost:45454").unwrap());
            if conn.is_err() {
                return None;
            }

            let (mut socket, _r) = conn.unwrap();

            // this should really be done by serde, but we're only going to send 1 message for now
            let msg = r#"{"request-type":"GetStreamingStatus", "message-id":"0"}"#.to_string();
            socket
                .write_message(ws::Message::Text(msg))
                .expect("must be able to write this");

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
                let ts = timecode.to_string();
                tx.send(ts);
                Some(())
            } else {
                None
            }
        }

        let (tx, rx) = crossbeam_channel::bounded(1);
        let tx = tx.clone();
        ::std::thread::spawn(move || {
            if get_inner(&tx).is_none() {
                drop(tx)
            }
        });

        use crossbeam_channel::after;
        let timeout = ::std::time::Duration::from_millis(150);
        select!{
                recv(rx, msg) => msg,
                recv(after(timeout)) => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::*;

    #[test]
    fn test_autojoin_raw() {
        let env = Environment::new();
        let _module = Builtin::new(&env.bot, &env.config);

        env.conn.push(":test.localhost 001 museun :Welcome to IRC");
        env.step();

        assert_eq!(env.conn.pop(), Some("JOIN #museun".into()))
    }

    #[test]
    fn test_ping_raw() {
        let env = Environment::new();
        let _module = Builtin::new(&env.bot, &env.config);

        env.conn.push("PING :foobar");
        env.step();

        assert_eq!(env.conn.pop(), Some("PONG :foobar".into()))
    }

    #[test]
    fn test_version_command() {
        let env = Environment::new();
        let _module = Builtin::new(&env.bot, &env.config);

        env.push_privmsg("!version");
        env.step();

        assert!(
            env.pop_env()
                .unwrap()
                .data
                .starts_with("https://github.com/museun/shaken")
        );
    }

    #[test]
    fn test_shaken_command() {
        let env = Environment::new();
        let _module = Builtin::new(&env.bot, &env.config);

        env.push_privmsg("!shaken");
        env.step();

        assert_eq!(
            env.pop_env().unwrap().data,
            "I try to impersonate The Bard, by being trained on all of his works."
        );
    }

    #[test]
    fn test_commands_command() {
        let env = Environment::new();
        let _module = Builtin::new(&env.bot, &env.config);

        env.push_privmsg("!commands");
        env.step();

        assert!(
            env.pop_env()
                .unwrap()
                .data
                .starts_with("available commands:")
        );
    }

    #[test]
    #[ignore]
    fn test_viewers_command() {
        // this requires connecting to twitch. would need mocking
    }

    #[test]
    #[ignore]
    fn test_uptime_command() {
        // this requires connecting to twitch
        // also requires obs is running
        // would need mocking

    }
}
