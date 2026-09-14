use crate::{
    Error, ErrorKind,
    lexer::{Kind, Lexer, Token},
    vm::Instruction,
};

const MAX_SOURCE_BYTES: usize = 1024 * 1024;
const MAX_PARSE_DEPTH: usize = 128;

pub(crate) fn compile(source: &str) -> Result<Vec<Instruction>, Error> {
    if source.len() > MAX_SOURCE_BYTES {
        return Err(Error::new(
            ErrorKind::ResourceLimit,
            MAX_SOURCE_BYTES,
            "Source exceeds the 1 MiB limit.",
        ));
    }
    let mut lexer = Lexer::new(source);
    let current = lexer.next()?;
    let mut parser = Parser {
        lexer,
        current,
        code: Vec::new(),
    };
    if parser.current.kind == Kind::End {
        return Ok(parser.code);
    }
    if parser.current.kind == Kind::Semicolon {
        return Err(parser.error(
            ErrorKind::Unsupported,
            "Empty statements are not implemented.",
        ));
    }
    parser.expression(0, 0)?;
    let terminated = parser.current.kind == Kind::Semicolon;
    if terminated {
        parser.advance()?;
    }
    if parser.current.kind != Kind::End {
        if terminated
            || (parser.current.line_break_before && matches!(parser.current.kind, Kind::Number(_)))
        {
            return Err(parser.error(
                ErrorKind::Unsupported,
                "Statement lists and automatic semicolon insertion are not implemented.",
            ));
        }
        return Err(parser.error(ErrorKind::Syntax, "Unexpected trailing token."));
    }
    Ok(parser.code)
}

struct Parser<'a> {
    lexer: Lexer<'a>,
    current: Token,
    code: Vec<Instruction>,
}

impl Parser<'_> {
    fn advance(&mut self) -> Result<(), Error> {
        self.current = self.lexer.next()?;
        Ok(())
    }

    fn error(&self, kind: ErrorKind, message: &'static str) -> Error {
        Error::new(kind, self.current.offset, message)
    }

    fn expression(&mut self, minimum: u8, depth: usize) -> Result<(), Error> {
        if depth >= MAX_PARSE_DEPTH {
            return Err(self.error(
                ErrorKind::ResourceLimit,
                "Parser recursion exceeds the 128-level limit.",
            ));
        }
        match self.current.kind {
            Kind::Number(number) => {
                self.code.push(Instruction::Number(number));
                self.advance()?;
            }
            Kind::Plus | Kind::Minus => {
                let negative = self.current.kind == Kind::Minus;
                self.advance()?;
                self.expression(3, depth + 1)?;
                self.code.push(if negative {
                    Instruction::Negative
                } else {
                    Instruction::Positive
                });
            }
            Kind::OpenParen => {
                self.advance()?;
                self.expression(0, depth + 1)?;
                if self.current.kind != Kind::CloseParen {
                    return Err(self.error(ErrorKind::Syntax, "Expected closing parenthesis."));
                }
                self.advance()?;
            }
            Kind::Slash => {
                return Err(self.error(
                    ErrorKind::Unsupported,
                    "Regular expression literals are not implemented.",
                ));
            }
            _ => {
                return Err(self.error(
                    ErrorKind::Syntax,
                    "Expected a number or parenthesized expression.",
                ));
            }
        }
        loop {
            let (precedence, instruction) = match self.current.kind {
                Kind::Plus => (1, Instruction::Add),
                Kind::Minus => (1, Instruction::Subtract),
                Kind::Star => (2, Instruction::Multiply),
                Kind::Slash => (2, Instruction::Divide),
                Kind::Percent => (2, Instruction::Remainder),
                Kind::OpenParen => {
                    return Err(self.error(
                        ErrorKind::Unsupported,
                        "Call expressions are not implemented.",
                    ));
                }
                _ => break,
            };
            if precedence < minimum {
                break;
            }
            self.advance()?;
            self.expression(precedence + 1, depth + 1)?;
            self.code.push(instruction);
        }
        Ok(())
    }
}
