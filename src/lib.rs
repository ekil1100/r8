#![forbid(unsafe_code)]

mod compiler;
mod lexer;
mod vm;

use std::fmt;

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
    Unsupported,
    ResourceLimit,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Error {
    pub kind: ErrorKind,
    pub offset: usize,
    pub message: &'static str,
}

impl Error {
    fn new(kind: ErrorKind, offset: usize, message: &'static str) -> Self {
        Self {
            kind,
            offset,
            message,
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self.kind {
            ErrorKind::Syntax => "SyntaxError",
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
}

impl Script {
    /// Parse and compile a Script without executing it.
    pub fn parse(source: &str) -> Result<Self, Error> {
        Ok(Self {
            code: compiler::compile(source)?,
        })
    }

    /// Execute compiler-produced instructions with a fresh operand stack.
    pub fn run(&self) -> Value {
        vm::run(&self.code)
    }
}

pub fn eval(source: &str) -> Result<Value, Error> {
    Ok(Script::parse(source)?.run())
}
