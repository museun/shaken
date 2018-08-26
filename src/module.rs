use parking_lot::RwLock;
use std::{
    sync::Arc,
    thread,
    time::{Duration, Instant},
};

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
    fn tick(&self, _dt: Instant) -> Option<Response> {
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

#[macro_export]
macro_rules! require_owner {
    ($req:expr) => {{
        if !$req.is_from_owner() {
            return None;
        };
        $req
    }};
    ($req:expr, $reason:expr) => {{
        if !$req.is_from_owner() {
            return reply!($reason);
        };
        $req
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

// TODO this should be using $crate instead of the FQN
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
