use crate::prelude::*;
use log::*;

// this is used so modules can express their commands
pub struct Command<T>
where
    T: Module + ?Sized,
{
    name: String,
    func: fn(&mut T, &Request) -> Option<Response>,
}

impl<T> Command<T>
where
    T: Module,
{
    pub fn new<S>(name: S, func: fn(&mut T, &Request) -> Option<Response>) -> Self
    where
        S: Into<String>,
    {
        Self {
            name: name.into(),
            func,
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn call(&self, recv: &mut T, req: &Request) -> Option<Response> {
        trace!("calling");
        (self.func)(recv, req)
    }
}
