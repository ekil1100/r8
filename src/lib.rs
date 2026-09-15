#![forbid(unsafe_code)]

mod compiler;
mod environment;
mod lexer;
mod vm;

use std::fmt;

pub use environment::Context;

pub const MAX_SOURCE_BYTES: usize = 1024 * 1024;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Value {
    Undefined,
    Number(f64),
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Undefined => f.write_str("undefined"),
            Self::Number(number) => f.write_str(ryu_js::Buffer::new().format(*number)),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ErrorKind {
    Syntax,
    Reference,
    Unsupported,
    ResourceLimit,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Error {
    pub kind: ErrorKind,
    pub offset: usize,
    pub message: String,
}

impl Error {
    fn new(kind: ErrorKind, offset: usize, message: impl Into<String>) -> Self {
        Self {
            kind,
            offset,
            message: message.into(),
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self.kind {
            ErrorKind::Syntax => "SyntaxError",
            ErrorKind::Reference => "ReferenceError",
            ErrorKind::Unsupported => "Unsupported",
            ErrorKind::ResourceLimit => "ResourceLimit",
        };
        write!(f, "{name} at byte {}: {}", self.offset, self.message)
    }
}

impl std::error::Error for Error {}

#[derive(Debug)]
pub struct Script {
    code: Vec<vm::Instruction>,
    lexical: Vec<environment::Declaration>,
    variables: Vec<environment::Declaration>,
}

impl Script {
    /// Parse and compile a Script without executing it.
    pub fn parse(source: &str) -> Result<Self, Error> {
        compiler::compile(source)
    }

    /// Execute in a fresh context. Use Context::run to share global bindings.
    pub fn run(&self) -> Result<Value, Error> {
        Context::default().run(self)
    }
}

pub fn eval(source: &str) -> Result<Value, Error> {
    Script::parse(source)?.run()
}
