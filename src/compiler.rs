use std::collections::BTreeSet;

use crate::{
    Error, ErrorKind, MAX_SOURCE_BYTES, Script,
    environment::{Declaration, DeclarationKind},
    lexer::{Kind, Lexer, Token},
    vm::Instruction,
};

const MAX_PARSE_DEPTH: usize = 128;

pub(crate) fn compile(source: &str) -> Result<Script, Error> {
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
        scopes: vec![Scope::default()],
        variables: Vec::new(),
    };
    while parser.current.kind != Kind::End {
        parser.statement(0)?;
    }
    Ok(Script {
        code: parser.code,
        lexical: parser
            .scopes
            .pop()
            .expect("Script scope must exist")
            .declarations,
        variables: parser.variables,
    })
}

#[derive(Default)]
struct Scope {
    declarations: Vec<Declaration>,
    lexical_names: BTreeSet<String>,
    var_names: BTreeSet<String>,
}

struct Parser<'a> {
    lexer: Lexer<'a>,
    current: Token,
    code: Vec<Instruction>,
    scopes: Vec<Scope>,
    variables: Vec<Declaration>,
}

impl Parser<'_> {
    fn advance(&mut self) -> Result<(), Error> {
        self.current = self.lexer.next()?;
        Ok(())
    }

    fn error(&self, kind: ErrorKind, message: &'static str) -> Error {
        Error::new(kind, self.current.offset, message)
    }

    fn depth(&self, depth: usize) -> Result<(), Error> {
        if depth >= MAX_PARSE_DEPTH {
            return Err(self.error(
                ErrorKind::ResourceLimit,
                "Parser recursion exceeds the 128-level limit.",
            ));
        }
        Ok(())
    }

    fn statement(&mut self, depth: usize) -> Result<(), Error> {
        self.depth(depth)?;
        match &self.current.kind {
            Kind::Semicolon => self.advance(),
            Kind::OpenBrace => self.block(depth + 1),
            Kind::Identifier(name) if name == "var" && !self.current.escaped => {
                self.declaration(DeclarationKind::Var, depth)
            }
            Kind::Identifier(name) if name == "const" && !self.current.escaped => {
                self.declaration(DeclarationKind::Const, depth)
            }
            Kind::Identifier(name)
                if name == "let"
                    && !self.current.escaped
                    && self.starts_lexical_declaration()? =>
            {
                self.declaration(DeclarationKind::Let, depth)
            }
            _ => {
                self.expression(0, depth)?;
                self.semicolon()?;
                self.code.push(Instruction::Complete);
                Ok(())
            }
        }
    }

    fn block(&mut self, depth: usize) -> Result<(), Error> {
        self.depth(depth)?;
        self.advance()?;
        self.scopes.push(Scope::default());
        let entry = self.code.len();
        self.code.push(Instruction::EnterScope(Vec::new()));
        while self.current.kind != Kind::CloseBrace {
            if self.current.kind == Kind::End {
                return Err(self.error(ErrorKind::Syntax, "Expected closing brace."));
            }
            self.statement(depth)?;
        }
        self.advance()?;
        let scope = self.scopes.pop().expect("Block scope must exist");
        self.code[entry] = Instruction::EnterScope(scope.declarations);
        self.code.push(Instruction::LeaveScope);
        Ok(())
    }

    fn starts_lexical_declaration(&self) -> Result<bool, Error> {
        let token = self.lexer.clone().next()?;
        Ok(match &token.kind {
            Kind::Identifier(name) => !reserved(name),
            Kind::OpenBrace => true,
            _ => false,
        })
    }

    fn semicolon(&mut self) -> Result<(), Error> {
        if self.current.kind == Kind::Semicolon {
            self.advance()
        } else if matches!(self.current.kind, Kind::End | Kind::CloseBrace)
            || self.current.line_break_before
        {
            Ok(())
        } else if matches!(self.current.kind, Kind::Assign | Kind::Comma) {
            Err(self.error(
                ErrorKind::Unsupported,
                "Assignment and sequence expressions are not implemented.",
            ))
        } else {
            Err(self.error(
                ErrorKind::Syntax,
                "Expected a semicolon or line terminator.",
            ))
        }
    }

    fn declaration(&mut self, kind: DeclarationKind, depth: usize) -> Result<(), Error> {
        self.advance()?;
        loop {
            let offset = self.current.offset;
            let name = match &self.current.kind {
                Kind::Identifier(name)
                    if !reserved(name) && !(kind != DeclarationKind::Var && name == "let") =>
                {
                    name.clone()
                }
                Kind::OpenBrace => {
                    return Err(self.error(
                        ErrorKind::Unsupported,
                        "Binding patterns are not implemented.",
                    ));
                }
                _ => return Err(self.error(ErrorKind::Syntax, "Expected a binding identifier.")),
            };
            self.declare(Declaration {
                name: name.clone(),
                kind,
                offset,
            })?;
            self.advance()?;
            if self.current.kind == Kind::Assign {
                self.advance()?;
                self.expression(0, depth)?;
                self.code.push(if kind == DeclarationKind::Var {
                    Instruction::SetVar(name)
                } else {
                    Instruction::Initialize(name)
                });
            } else if kind == DeclarationKind::Const {
                return Err(self.error(
                    ErrorKind::Syntax,
                    "A const declaration requires an initializer.",
                ));
            } else if kind == DeclarationKind::Let {
                self.code.push(Instruction::Undefined);
                self.code.push(Instruction::Initialize(name));
            }
            if self.current.kind != Kind::Comma {
                return self.semicolon();
            }
            self.advance()?;
        }
    }

    fn declare(&mut self, declaration: Declaration) -> Result<(), Error> {
        let name = &declaration.name;
        if declaration.kind == DeclarationKind::Var {
            if self
                .scopes
                .iter()
                .any(|scope| scope.lexical_names.contains(name))
            {
                return Err(self.error(
                    ErrorKind::Syntax,
                    "Variable declaration conflicts with a lexical binding.",
                ));
            }
            let first = !self.scopes[0].var_names.contains(name);
            for scope in &mut self.scopes {
                scope.var_names.insert(name.clone());
            }
            if first {
                self.variables.push(declaration);
            }
        } else {
            let scope = self.scopes.last_mut().expect("Current scope must exist");
            if scope.var_names.contains(name) || !scope.lexical_names.insert(name.clone()) {
                return Err(self.error(
                    ErrorKind::Syntax,
                    "Duplicate or conflicting lexical declaration.",
                ));
            }
            scope.declarations.push(declaration);
        }
        Ok(())
    }

    fn expression(&mut self, minimum: u8, depth: usize) -> Result<(), Error> {
        self.depth(depth)?;
        match self.current.kind.clone() {
            Kind::Number(number) => {
                self.code.push(Instruction::Number(number));
                self.advance()?;
            }
            Kind::Identifier(name) => {
                if reserved(&name) {
                    if self.current.escaped {
                        return Err(
                            self.error(ErrorKind::Syntax, "Keywords cannot contain escapes.")
                        );
                    }
                    return Err(self.error(
                        ErrorKind::Unsupported,
                        "This keyword's syntax is not implemented.",
                    ));
                }
                let offset = self.current.offset;
                self.advance()?;
                if matches!(name.as_str(), "async" | "using" | "await")
                    && !self.current.line_break_before
                    && matches!(self.current.kind, Kind::Identifier(_))
                {
                    return Err(self.error(
                        ErrorKind::Unsupported,
                        "Async and resource-management syntax is not implemented.",
                    ));
                }
                self.code.push(Instruction::Load(name, offset));
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
                    if matches!(self.current.kind, Kind::Assign | Kind::Comma) {
                        return Err(self.error(
                            ErrorKind::Unsupported,
                            "Assignment and sequence expressions are not implemented.",
                        ));
                    }
                    return Err(self.error(ErrorKind::Syntax, "Expected closing parenthesis."));
                }
                self.advance()?;
            }
            Kind::OpenBrace => {
                return Err(self.error(
                    ErrorKind::Unsupported,
                    "Object literals are not implemented.",
                ));
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
                    "Expected a number, identifier or parenthesized expression.",
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
                Kind::Assign => {
                    return Err(self.error(
                        ErrorKind::Unsupported,
                        "Assignment expressions are not implemented.",
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

pub(crate) fn reserved(name: &str) -> bool {
    matches!(
        name,
        "break"
            | "case"
            | "catch"
            | "class"
            | "const"
            | "continue"
            | "debugger"
            | "default"
            | "delete"
            | "do"
            | "else"
            | "enum"
            | "export"
            | "extends"
            | "false"
            | "finally"
            | "for"
            | "function"
            | "if"
            | "import"
            | "in"
            | "instanceof"
            | "new"
            | "null"
            | "return"
            | "super"
            | "switch"
            | "this"
            | "throw"
            | "true"
            | "try"
            | "typeof"
            | "var"
            | "void"
            | "while"
            | "with"
    )
}
