use crate::*;

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
    pub fn new(name: &str, func: fn(&T, &Request) -> Option<Response>) -> Self {
        Self {
            name: name.into(),
            func,
        }
    }

    pub fn name(&self) -> &str { &self.name }

    pub fn call(&self, recv: &T, req: &Request) -> Option<Response> {
        trace!("calling");
        (self.func)(recv, req)
    }
}

#[macro_export]
macro_rules! command_list {
    ($(($name:expr,$cmd:expr)),* $(,)*) => {{
        let mut list = Vec::new();
        $(
            list.push($crate::Command::new($name, $cmd));
        )*
        // TODO: impl ord on commands
        list.sort_unstable_by(|a,b| b.name().len().cmp(&a.name().len()));
        list
    }};
}
