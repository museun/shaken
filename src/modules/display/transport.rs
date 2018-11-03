use crate::prelude::*;

// TODO associate type for an error
pub trait Transport: Send {
    fn send(&self, msg: Message);
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Message {
    pub userid: String,
    pub timestamp: u64,
    pub color: RGB,
    pub name: String,
    pub data: String,
    pub badges: Vec<irc::Badge>,
    pub kappas: Vec<irc::Kappa>,
    pub is_action: bool,
}

impl Message {
    pub fn new(msg: &irc::Message, user: User) -> Self {
        let (data, is_action) = if msg.data.starts_with('\x01') {
            (msg.data[8..msg.data.len() - 1].to_string(), true)
        } else {
            (msg.data.clone(), false)
        };

        let ts = util::get_timestamp();
        Self {
            userid: user.userid.to_string(),
            timestamp: ts,
            color: user.color,
            name: user.display,
            data,
            kappas: msg.tags.get_kappas().unwrap_or_default(),
            badges: msg.tags.get_badges().unwrap_or_default(),
            is_action,
        }
    }
}
