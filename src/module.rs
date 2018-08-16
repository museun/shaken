use parking_lot::RwLock;
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use crate::irc::Message;
use crate::*;

pub trait Module {
    fn command(&self, req: &Request) -> Option<Response> {
        None
    }

    fn passive(&self, _msg: &Message) -> Option<Response> {
        None
    }

    fn event(&self, _msg: &Message) -> Option<Response> {
        None
    }
}

#[macro_export]
macro_rules! dispatch_commands {
    ($this:expr, $req:expr) => {{
        for cmd in &$this.commands {
            if let Some(req) = $req.search(cmd.name()) {
                trace!("calling '{}' with {:?}", cmd.name(), &req);
                return cmd.call($this, &req);
            }
        }
        None
    }};
}

pub struct Every(crossbeam_channel::Sender<()>);

impl Every {
    pub fn new<T, F>(ctx: Arc<RwLock<T>>, f: F, ms: u64) -> Self
    where
        T: Send + Sync + 'static,
        F: Fn(Arc<RwLock<T>>, Instant) + Send + Sync + 'static,
    {
        let (tx, rx) = crossbeam_channel::bounded(0);
        let tick = crossbeam_channel::tick(Duration::from_millis(ms));

        let f = Arc::new(RwLock::new(f));
        thread::spawn(move || loop {
            select!{
                recv(tick, dt) => {
                    if let Some(dt) = dt {
                        let ctx = Arc::clone(&ctx);
                        (*f.write())(ctx, dt);
                    }
                    // when is a None sent?
                }
                recv(rx, _) => { return; }
            }
        });

        Self { 0: tx }
    }
}

impl Drop for Every {
    fn drop(&mut self) {
        self.0.send(())
    }
}

#[macro_export]
macro_rules! every {
    ($func:expr) => {
        every!($func, (), 1000)
    };

    // default to one second
    ($func:expr, $this:expr) => {
        every!($func, $this, 1000)
    };

    ($func:expr, $this:expr, $dur:expr) => {{
        let this = ::std::sync::Arc::new(parking_lot::RwLock::new($this));
        (
            ::std::sync::Arc::clone(&this),
            Every::new(::std::sync::Arc::clone(&this), $func, $dur),
        )
    }};
}
