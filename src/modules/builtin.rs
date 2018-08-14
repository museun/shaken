use {
    command::Command,
    module::Module,
    request::Request,
    response::{self, Response},
};
use {
    config::Config,
    irc::Message,
    twitch::{self, *},
    util::*,
};

use tungstenite;

use chrono::prelude::*;

pub struct Builtin {
    twitch: twitch::TwitchClient,
    channel: String,
    commands: Vec<Command<Builtin>>,
}

impl Module for Builtin {
    fn command(&self, req: &Request) -> Option<Response> {
        for cmd in &self.commands {
            if let Some(req) = req.search(cmd.name()) {
                return cmd.call(&self, &req);
            }
        }

        None
    }

    fn event(&self, msg: &Message) -> Option<Response> {
        match msg.command() {
            "001" => response::join(&self.channel),
            "PING" => raw!("PONG :{}", &msg.data),
            _ => None,
        }
    }
}

macro_rules! maybe {
    ($e:expr) => {
        if $e {
            return reply!("the stream doesn't seem to be live");
        }
    };
}

impl Builtin {
    pub fn new() -> Self {
        let commands = vec![
            Command::new("!version", Builtin::version_command),
            Command::new("!shaken", Builtin::shaken_command),
            Command::new("!viewers", Builtin::viewers_command),
            Command::new("!uptime", Builtin::uptime_command),
            // crate::simple_command("!uptime"),  //
        ];

        Self {
            twitch: twitch::TwitchClient::new(),
            commands,
            channel: Config::load().twitch.channel.to_string(),
        }
    }

    fn version_command(&self, _req: &Request) -> Option<Response> {
        let rev = option_env!("SHAKEN_GIT_REV").unwrap();
        let branch = option_env!("SHAKEN_GIT_BRANCH").unwrap();

        reply!(
            "https://github.com/museun/shaken ({} on '{}' branch)",
            rev,
            branch
        )
    }

    fn shaken_command(&self, _req: &Request) -> Option<Response> {
        say!("I try to impersonate The Bard, by being trained on all of his works.")
    }

    fn viewers_command(&self, _req: &Request) -> Option<Response> {
        let streams = self.twitch.get_streams(&[&self.channel]);
        maybe!(streams.is_none());

        let streams = streams.unwrap();
        maybe!(streams.is_empty());

        let stream = &streams[0];
        maybe!(stream.live.is_empty() || stream.live == "");

        let viewers = stream.viewer_count.comma_separate();
        reply!("viewers: {}", viewers)
    }

    fn uptime_command(&self, _req: &Request) -> Option<Response> {
        let timecode = Self::get_uptime_from_obs();

        let streams = self.twitch.get_streams(&[&self.channel]);
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

            let timecode = ::util::format_time_map(&map);
            msg.push_str(&format!(", obs says: {}", &timecode));
        }

        say!("{}", msg)
    }

    fn get_uptime_from_obs() -> Option<String> {
        fn get_inner(tx: &crossbeam_channel::Sender<String>) -> Option<()> {
            let conn = tungstenite::connect(::url::Url::parse("ws://localhost:45454").unwrap());
            if conn.is_err() {
                return None;
            }

            let (mut socket, _r) = conn.unwrap();

            // this should really be done by serde, but we're only going to send 1 message for now
            let msg = r#"{"request-type":"GetStreamingStatus", "message-id":"0"}"#.to_string();
            socket
                .write_message(tungstenite::Message::Text(msg))
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
        let timeout = ::std::time::Duration::from_millis(3000);
        select!{
                recv(rx, msg) => msg,
                recv(after(timeout)) =>{
                    warn!("timed out when trying to get the uptime from obs");
                    None
                },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use testing::*;

    #[test]
    fn autojoin() {
        let builtin: Box<dyn Module> = Box::new(Builtin::new());
        let mut env = Environment::new();
        env.add(&builtin);

        env.push_raw(":test.localhost 001 museun :Welcome to IRC");
        env.step();
        assert_eq!(env.pop_raw(), Some("JOIN museun".into()));
    }

    #[test]
    fn autopong() {
        let builtin: Box<dyn Module> = Box::new(Builtin::new());
        let mut env = Environment::new();
        env.add(&builtin);

        env.push_raw("PING :foobar");
        env.step();
        assert_eq!(env.pop_raw(), Some("PONG :foobar".into()));
    }

    #[test]
    fn shaken_command() {
        let builtin: Box<dyn Module> = Box::new(Builtin::new());
        let mut env = Environment::new();
        env.add(&builtin);

        env.push("!shaken");
        env.step();
        assert_eq!(
            env.pop(),
            Some("I try to impersonate The Bard, by being trained on all of his works.".into())
        );
    }

    #[test]
    fn version_command() {
        let builtin: Box<dyn Module> = Box::new(Builtin::new());
        let mut env = Environment::new();
        env.add(&builtin);

        env.push("!version");
        env.step();

        assert!(
            env.pop()
                .unwrap()
                .starts_with("@test: https://github.com/museun/shaken")
        );
    }

    #[test]
    #[ignore] // this requires mocking a twitch response
    fn viewers_command() {
        let builtin: Box<dyn Module> = Box::new(Builtin::new());
        let mut env = Environment::new();
        env.add(&builtin);

        env.push("!viewers");
        env.step();

        warn!("{:#?}", env.pop());
    }

    #[test]
    #[ignore] // this requires mocking a twitch response, and an obs response
    fn uptime_command() {
        init_logger();

        let builtin: Box<dyn Module> = Box::new(Builtin::new());
        let mut env = Environment::new();
        env.add(&builtin);

        env.push("!uptime");
        env.step();

        warn!("{:#?}", env.pop());
    }
}
