mod message;
pub use self::message::{Message, Prefix};

mod conn;
// TODO this should only expose a Conn
// with a generic constraint to select which underlying conn is used
pub use self::conn::{Conn, TcpConn, TestConn};
