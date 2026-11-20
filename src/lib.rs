//! jevbrev: short API definitions that code turns into Jev questions, answer readers and
//! HTTP calls.
//!
//! A `.brev` file describes an HTTP API in the words a person would use to ask for each
//! action. This crate reads it and, for one person's request, writes the two Jev
//! (TypeSafe System One) request bodies, reads Jev's answers into the inputs the request
//! said, and builds the HTTP call. It never calls Jev and never holds a key.
//!
//! ```
//! let brev = jevbrev::load(include_str!("../examples/clinic.brev")).unwrap();
//! let call1 = jevbrev::menu(&brev, "move Sam's Mondays to Tuesday", None);
//! assert_eq!(call1["questions"]["action"]["type"], "choice");
//! ```
//!
//! The format and every step are specified in `docs/using-brev.md`.
#![forbid(unsafe_code)]

pub mod frames;
pub mod import;
pub mod model;
pub mod out;
pub mod parse;
pub mod read;
pub mod spot;

#[cfg(feature = "wasm")]
mod wasm;

pub use frames::{FrameError, Known, MODEL, NONE, frames, menu};
pub use import::import_openapi;
pub use model::{Action, Api, Brev, Diag, Input, Kind, Role, RoleGroup, Type};
pub use out::{http_call, llm, openapi, tools};
pub use parse::{check, load, parse};
pub use read::{
    ActionPick, InputRead, Read, ReadError, State, THRESHOLD, ask_for, read, read_action,
};
