//! Error types

use num_derive::FromPrimitive;
use solana_sdk::{decode_error::DecodeError, program_error::ProgramError};
use thiserror::Error;

/// Errors that may be returned by the Themis program.
#[derive(Clone, Debug, Eq, Error, FromPrimitive, PartialEq)]
pub enum ThemisError {
    /// Invalid instruction
    #[error("Invalid instruction")]
    InvalidInstruction,

    /// Account already in use
    #[error("Account in use")]
    AccountInUse,
}
impl From<ThemisError> for ProgramError {
    fn from(e: ThemisError) -> Self {
        ProgramError::Custom(e as u32)
    }
}
impl<T> DecodeError<T> for ThemisError {
    fn type_of() -> &'static str {
        "ThemisError"
    }
}
