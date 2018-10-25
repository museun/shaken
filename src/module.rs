use std::{    sync::Arc,    thread,    time::{Duration, Instant},};
use parking_lot::RwLock;

use crate::prelude::*;

pub trait Module {
    fn command(&self, _req: &Request<'_>) -> Option<Response> {
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
