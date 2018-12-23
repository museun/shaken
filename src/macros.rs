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
                vec.push(arg);
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
macro_rules! require_priviledges {
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
