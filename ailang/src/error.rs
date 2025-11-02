use crate::token::OwnedToken;

#[derive(Debug, thiserror::Error, Clone)]
#[allow(dead_code)]
pub enum Error {
    #[error("[line {line}] {msg}")]
    Lex{line: usize, msg: String},
    #[error("[end] {msg}")]
    EndOfStream{msg: String},
    #[error("[line {line}] {msg}", line = tok.line)]
    Parse{tok: OwnedToken, msg: String},
    #[error("[line {line}] {msg}")]
    Compile{line: usize, msg: String},

    #[error("A callable named '{0}' is already registered")]
    DuplicateCallable(String),
    #[error("A property named '{0}' is already registered")]
    DuplicateProperty(String),

    #[error("No callable named '{0}' was registered")]
    UnknownCallable(String),
    #[error("No property named '{0}' was registered")]
    UnknownProperty(String),
    #[error("The '{0}' property was not declared with a 'use' statement")]
    UndeclaredProperty(String),

    #[error("[address {0}] '{1}' is not a settable property")]
    UnsettableProperty(usize, String),
    #[error("[address {0}] '{1}' is not a registered property")]
    UnregisteredProperty(usize, String),
    #[error("[address {0}] '{1}' is not a registered callable")]
    UnregisteredCallable(usize, String),
    #[error("[address {0}] Only built-in callables can be called in parallel")]
    InvalidCall(usize),

    #[error("{0} is not a valid Callable")]
    InvalidCallable(String),

    #[error("[address {0}] Stack underflow")]
    StackUnderflow(usize),
    // #[error("[address {0}] Stack overflow")]
    // StackOverflow(usize),
    #[error("[address {0}] Attempt to index outside of the stack")]
    IndexOutOfBounds(usize),
    #[error("{0}")]
    Type(String),


    #[error("Cannot modify compiler state while it's running")]
    CompilerActive,
    #[error("Cannot modify interpreter state while it's running")]
    InterpreterActive,

    #[error("Something went wrong while accessing from multiple threads. Consider not doing that.")]
    ThreadingError,

    // #[error("Value you at position {0} should be '{1}'")]
    // Call(usize, String),
    #[error("{0}")]
    Call(String),

    #[error("[line {line}] {msg}")]
    IRParse{line: usize, msg: String},

    #[error("{0}")]
    #[allow(dead_code)]
    Foreign(String),
}

pub type Result<T> = std::result::Result<T, Error>;
