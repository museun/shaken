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
    (@one $($x:tt)*) => (());
    (@len $($e:expr),*) => (<[()]>::len(&[$(multi!(@one $e)),*]));
    ($($arg:expr),* $(,)*) => {{
        use crate::prelude::Response;
        let mut vec = Vec::with_capacity(multi!(@len $($arg),*));
        $( if let Some(arg) = $arg { vec.push(arg); } )*
        Some(Response::Multi{data: vec})
    }};
}

#[macro_export]
macro_rules! reply {
    ($f:expr, $($arg:tt)*) => {{
        use crate::prelude::Response;
        Some(Response::Reply{data: format!($f, $($arg)*)})
    }};
    ($e:expr) => {{
        use crate::prelude::Response;
        Some(Response::Reply{data: $e.to_string()})
    }}
}

#[macro_export]
macro_rules! say {
    ($f:expr, $($arg:tt)*) => {{
        use crate::prelude::Response;
        Some(Response::Say{data: format!($f, $($arg)*)})
    }};
    ($e:expr) => {{
        use crate::prelude::Response;
        Some(Response::Say{data: $e.to_string()})
    }}
}

#[macro_export]
macro_rules! action {
    ($f:expr, $($arg:tt)*) => {{
        use crate::prelude::Response;
        Some(Response::Action{data: format!($f, $($arg)*)})
    }};
    ($e:expr) => {{
        use crate::prelude::Response;
        Some(Response::Action{data: $e.to_string()})
    }};
}

#[macro_export]
macro_rules! whisper {
    ($f:expr, $($arg:tt)*) => {{
        use crate::prelude::Response;
        Some(Response::Whisper{data: format!($f, $($arg)*)})
    }};
    ($e:expr) => {{
        use crate::prelude::Response;
        Some(Response::Whisper{data: $e.to_string()})
    }};
}

#[macro_export]
macro_rules! raw {
    ($f:expr, $($arg:tt)*) => {{
        use crate::prelude::Response;
        Some(Response::Command{cmd: IrcCommand::Raw{ data: format!($f, $($arg)*)}})
    }};
    ($e:expr) => {{
        use crate::prelude::Response;
        Some(Response::Command{cmd: IrcCommand::Raw{ data: $e.to_string()}})
    }};
}

#[macro_export]
macro_rules! privmsg {
    ($target:expr, $f:expr, $($arg:tt)*) => {{
        use crate::prelude::{Response, IrcCommand};
        Some(Response::Command {
            cmd: IrcCommand::Privmsg{
                target: $target.to_string(),
                data: format!($f, $($arg)*)
            }
        })
    }};

    ($target:expr, $f:expr) => {{
        use crate::prelude::{Response, IrcCommand};
        Some(Response::Command {
            cmd: IrcCommand::Privmsg{
                target: $target.to_string(),
                data: $f.to_string(),
            }
        })
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
macro_rules! require_broadcaster {
    ($req:expr) => {{
        if !$req.is_from_broadcaster() {
            return None;
        };
        $req
    }};
    ($req:expr, $reason:expr) => {{
        if !$req.is_from_broadcaster() {
            return reply!($reason);
        };
        $req
    }};
}

#[macro_export]
macro_rules! require_moderator {
    ($req:expr) => {{
        if !$req.is_from_moderator() {
            return None;
        };
        $req
    }};
    ($req:expr, $reason:expr) => {{
        if !$req.is_from_moderator() {
            return reply!($reason);
        };
        $req
    }};
}

#[macro_export]
macro_rules! require_privileges {
    ($req:expr) => {{
        if !$req.is_from_owner() && !$req.is_from_broadcaster() && !$req.is_from_moderator() {
            return None;
        };
        $req
    }};
    ($req:expr, $reason:expr) => {{
        if !$req.is_from_owner() && !$req.is_from_broadcaster() && !$req.is_from_moderator() {
            return reply!($reason);
        };
        $req
    }};
}

// unused but not forgotten
#[macro_export]
macro_rules! map {
    (@one $($x:tt)*) => (());
    (@len $($e:expr),*) => (<[()]>::len(&[$(map!(@one $e)),*]));
    ($($k:expr => $v:expr),*) => {{
        let mut _map = hashbrown::HashMap::with_capacity(map!(@len $($k),*));
        $( let _ = _map.insert($k.to_string(), $v.to_string()); )*
        _map
    }};
}
