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
        S: ToString,
    {
        Self {
            name: name.to_string(),
            func,
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn call(&self, recv: &mut T, req: &Request) -> Option<Response> {
        debug!("calling command: {}", self.name);
        (self.func)(recv, req)
    }
}
