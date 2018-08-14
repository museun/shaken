use crate::irc::Message;
use crate::*;

pub trait Module {
    fn command(&self, _req: &Request) -> Option<Response> {
        None
    }

    fn passive(&self, _msg: &Message) -> Option<Response> {
        None
    }

    fn event(&self, _msg: &Message) -> Option<Response> {
        None
    }
}
