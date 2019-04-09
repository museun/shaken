pub mod database;
pub mod error;
pub mod local;
pub mod server;
pub mod youtube;

// TODO don't do this
pub use self::database::*;
pub use self::error::*;
pub use self::local::*;
pub use self::server::*;
pub use self::youtube::*;

pub trait Storage<T>
where
    T: FromRow,
{
    fn insert(&self, item: &server::Item) -> Result<()>;
    fn current(&self) -> Result<T>;
    fn previous(&self) -> Result<T>;
    fn all(&self) -> Result<Vec<T>>;
}

pub trait FromRow {
    fn from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Self>
    where
        Self: Sized;
    fn timestamp(&self) -> i64;
}
