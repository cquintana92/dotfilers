#[macro_use]
extern crate tracing;

pub mod config;
pub mod executor;

pub use config::*;
pub use executor::*;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("config error: {0}")]
    Config(String),
}
