use crate::prelude::*;

use std::sync::{Arc, Mutex};

mod transport;
pub use self::transport::{Message, Transport};

pub mod transports;

pub struct Display {
    name: String,
    map: CommandMap<Display>,
    transports: Vec<Arc<Mutex<Transport>>>,
}

impl Module for Display {
    fn command(&mut self, req: &Request) -> Option<Response> {
        let map = self.map.shallow_clone();
        map.dispatch(self, req)
    }

    fn passive(&mut self, msg: &irc::Message) -> Option<Response> {
        // TODO: this only handles PRIVMSG
        if msg.command.as_str() == "PRIVMSG" {
            self.handle_passive(msg);
        }
        None
    }

    fn inspect(&mut self, msg: &irc::Message, resp: &Response) {
        self.inspect_event(msg, resp);
    }
}

// never forget
// .or_else::<HashMap<String, RGB>, _>(|_: Option<()>| Ok(HashMap::new()))
impl Display {
    pub fn create(transports: Vec<Arc<Mutex<Transport>>>) -> Result<Self, ModuleError> {
        let map = CommandMap::create("Display", &[("!color", Display::color_command)])?;
        let config = Config::load();

        Ok(Self {
            name: config.twitch.name.clone(),
            map,
            transports,
        })
    }

    fn color_command(&mut self, req: &Request) -> Option<Response> {
        let id = req.sender();
        let part = req.args_iter().next()?;

        let color = match part.to_ascii_lowercase().as_str() {
            "reset" => req.color(),
            _ => {
                let color = RGB::from(part);
                if color.is_dark() {
                    return reply!("don't use that color");
                }
                color
            }
        };

        let conn = database::get_connection();
        UserStore::update_color_for_id(&conn, id, &color);
        None
    }

    fn handle_passive(&self, msg: &irc::Message) -> Option<()> {
        let conn = database::get_connection();
        let user = UserStore::get_user_by_id(&conn, msg.tags.get_userid()?)?;

        if !msg.data.starts_with('!') {
            println!("<{}> {}", user.color.format(&user.display), &msg.data);
        }

        let msg = Message::new(msg, user);
        for transport in &self.transports {
            transport.lock().unwrap().send(msg.clone())
        }
        None
    }

    fn inspect_event(&self, msg: &irc::Message, resp: &Response) -> Option<()> {
        match resp {
            Response::Command { .. } | Response::Action { .. } | Response::Whisper { .. } => {
                return None
            }
            _ => {}
        };

        let conn = database::get_connection();
        let user = UserStore::get_bot(&conn, &self.name)?;
        let resp = resp.build(Some(msg))?;
        for out in resp {
            let msg = irc::Message::parse(&out);
            println!(
                "<{}> {}",                         //
                &user.color.format(&user.display), //
                &msg.data
            );

            let msg = Message::new(&msg, user.clone());
            for transport in &self.transports {
                transport.lock().unwrap().send(msg.clone())
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::*;

    #[test]
    fn color_command() {
        let db = database::get_connection();
        let mut display = Display::create(vec![]).unwrap();
        let mut env = Environment::new(&db, &mut display);

        env.push("!color #111111");
        env.step();
        assert_eq!(env.pop(), Some("@test: don't use that color".into()));

        env.push("!color #FFFFFF");
        env.step_wait(false);

        let conn = env.get_db_conn();
        let user = UserStore::get_user_by_id(&conn, 1000).unwrap();
        assert_eq!(user.color, RGB::from("#FFFFFF"));
    }
}
