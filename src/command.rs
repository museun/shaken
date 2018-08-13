use crate::{Module, Request, Response};

// this is used so modules can express their commands
pub struct Command<T>
where
    T: Module,
{
    name: String,
    func: fn(&T, &Request) -> Option<Response>,
}

impl<T> Command<T>
where
    T: Module,
{
    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn new(name: &str, func: fn(&T, &Request) -> Option<Response>) -> Self {
        Self {
            name: name[1..].into(),
            func,
        }
    }

    pub fn call(&self, recv: &T, req: &Request) -> Option<Response> {
        (self.func)(recv, req)
    }
}
