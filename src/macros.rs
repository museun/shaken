#[macro_export]
macro_rules! bail {
    ($e:expr) => {
        match $e {
            Some(item) => item,
            None => return,
        }
    };
}

#[macro_export]
macro_rules! multi {
    ($($arg:expr),* $(,)*) => {{
        use crate::prelude::Response;
        let mut vec = vec![];

        $(
            if let Some(arg) = $arg {
                vec.push(Box::new(arg));
            }
        )*

        Some(Response::Multi{data: vec})
    }};
}

#[macro_export]
macro_rules! reply {
    ($($arg:tt)*) => {{
        use crate::prelude::Response;
        Some(Response::Reply{data: format!($($arg)*)})
    }};
}

#[macro_export]
macro_rules! say {
    ($($arg:tt)*) => {{
        use crate::prelude::Response;
        Some(Response::Say{data: format!($($arg)*)})
    }}
}

#[macro_export]
macro_rules! action {
    ($($arg:tt)*) => {{
        use crate::prelude::Response;
        Some(Response::Action{data: format!($($arg)*)})
    }};
}

#[macro_export]
macro_rules! whisper {
    ($($arg:tt)*) => {{
        use crate::prelude::Response;
        Some(Response::Whisper{data: format!($($arg)*)})
    }};
}

#[macro_export]
macro_rules! raw {
    ($($arg:tt)*) => {{
        use crate::prelude::{Response, IrcCommand};
       Some(Response::Command{cmd: IrcCommand::Raw{ data: format!($($arg)*) }})
    }};
}

#[macro_export]
macro_rules! privmsg {
    ($target:expr, $($arg:tt)*) => {{
        use crate::prelude::{Response, IrcCommand};
        Some(Response::Command {
            cmd: IrcCommand::Privmsg{
                target: $target.to_string(),
                data: format!($($arg)*)
            }
        })
    }};
}

#[macro_export]
macro_rules! command_list {
    ($(($name:expr,$cmd:expr)),* $(,)*) => {{
        use crate::prelude::Command;
        let mut list = Vec::new();
        $(
            list.push(Command::new($name, $cmd));
        )*
        list.sort_unstable_by(|a,b| b.name().len().cmp(&a.name().len()));
        list
    }};
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
        use {crate::prelude::Every, std::sync::Arc};
        let this = Arc::new(parking_lot::RwLock::new($this));
        (
            Arc::clone(&this),
            Every::new(Arc::clone(&this), $func, $dur),
        )
    }};
}
