use bot::Bot;
use message::{Message, Prefix};

#[derive(Debug, Clone)]
pub struct Envelope {
    pub channel: String,
    pub sender: Prefix,
    pub data: String,
}

impl Envelope {
    pub fn from_msg(msg: &Message) -> Self {
        assert!(msg.command == "PRIVMSG");

        let msg = msg.clone();
        Self {
            channel: msg.args[0].to_string(),
            sender: msg.prefix.unwrap(),
            data: msg.data.to_string(),
        }
    }
}

pub enum Handler {
    Active(&'static str, fn(&Bot, &Envelope)),
    Passive(fn(&Bot, &Envelope)),
    Raw(&'static str, fn(&Bot, &Message)),
}
