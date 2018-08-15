use std::time::Duration;

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

pub struct Every {
    dur: Duration,
}

impl Every {
    pub fn new(secs: u64) -> Self {
        // start a thread
        // use a crossbeam channel to do stuff
        // think of the api for this
        // it'll probably not use function pointers

        Self {
            dur: Duration::from_secs(secs),
        }
    }
}
