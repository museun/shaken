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
}

impl Message {
    pub fn new(msg: &irc::Message, user: User) -> Self {
        let ts = util::get_timestamp();
        Self {
            userid: user.userid.to_string(),
            timestamp: ts,
            color: user.color,
            name: user.display,
            data: msg.data.clone(),
            kappas: msg.tags.get_kappas().unwrap_or_default(),
            badges: msg.tags.get_badges().unwrap_or_default(),
        }
    }
}
