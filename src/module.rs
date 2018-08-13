use crate::irc::Message;
use crate::{Command, Request, Response};

pub trait Module {
    fn command(&self, cmd: &Request) -> Option<Response> {
        None
    }

    fn passive(&self, msg: &Message) -> Option<Response> {
        None
    }

    fn event(&self, msg: &Message) -> Option<Response> {
        None
    }

    fn commands(&self) -> Vec<Command> {
        vec![]
    }
}
