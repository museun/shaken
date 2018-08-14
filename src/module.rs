use irc::Message;
use request::Request;
use response::Response;

pub trait Module {
    fn command(&self, req: &Request) -> Option<Response> {
        None
    }

    fn passive(&self, msg: &Message) -> Option<Response> {
        None
    }

    fn event(&self, msg: &Message) -> Option<Response> {
        None
    }
}
