#[macro_use]
mod util;
pub use crate::util::*;

mod train;
pub use crate::train::train;

mod host;
pub use crate::host::Server;

mod markov;
pub use crate::markov::Markov;

pub fn load<'a>(input: &str, buf: &'a [u8]) -> Markov<'a> {
    timeit!("loading {}", input);
    bincode::deserialize(&buf).expect("deserialize file")
}
