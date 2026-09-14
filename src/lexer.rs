use crate::{Error, ErrorKind};

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Kind {
    Number(f64),
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    OpenParen,
    CloseParen,
    Semicolon,
    End,
}

pub(crate) struct Token {
    pub kind: Kind,
    pub offset: usize,
    pub line_break_before: bool,
}

pub(crate) struct Lexer<'a> {
    source: &'a str,
    offset: usize,
}

impl<'a> Lexer<'a> {
    pub fn new(source: &'a str) -> Self {
        Self { source, offset: 0 }
    }

    pub fn next(&mut self) -> Result<Token, Error> {
        let line_break_before = self.trivia()?;
        let offset = self.offset;
        let Some(character) = self.source[offset..].chars().next() else {
            return Ok(Token {
                kind: Kind::End,
                offset,
                line_break_before,
            });
        };
        if character.is_ascii_digit()
            || (character == '.'
                && self
                    .source
                    .as_bytes()
                    .get(offset + 1)
                    .is_some_and(u8::is_ascii_digit))
        {
            return Ok(Token {
                kind: Kind::Number(self.number()?),
                offset,
                line_break_before,
            });
        }
        let rest = &self.source[offset..];
        if ["++", "--", "**", "+=", "-=", "*=", "/=", "%="]
            .iter()
            .any(|operator| rest.starts_with(operator))
        {
            return Err(Error::new(
                ErrorKind::Unsupported,
                offset,
                "Update, exponentiation and assignment operators are not implemented.",
            ));
        }
        self.offset += character.len_utf8();
        let kind = match character {
            '+' => Kind::Plus,
            '-' => Kind::Minus,
            '*' => Kind::Star,
            '/' => Kind::Slash,
            '%' => Kind::Percent,
            '(' => Kind::OpenParen,
            ')' => Kind::CloseParen,
            ';' => Kind::Semicolon,
            '@' => {
                return Err(Error::new(
                    ErrorKind::Syntax,
                    offset,
                    "Unexpected character.",
                ));
            }
            character if character.is_control() => {
                return Err(Error::new(
                    ErrorKind::Syntax,
                    offset,
                    "Unexpected control character.",
                ));
            }
            _ => {
                return Err(Error::new(
                    ErrorKind::Unsupported,
                    offset,
                    "Syntax outside the r8 arithmetic subset.",
                ));
            }
        };
        Ok(Token {
            kind,
            offset,
            line_break_before,
        })
    }

    fn trivia(&mut self) -> Result<bool, Error> {
        let mut line_break = false;
        loop {
            let rest = &self.source[self.offset..];
            match rest.chars().next() {
                Some(character) if is_whitespace(character) || is_line_terminator(character) => {
                    line_break |= is_line_terminator(character);
                    self.offset += character.len_utf8();
                }
                _ if rest.starts_with("//") => {
                    self.offset += rest.find(is_line_terminator).unwrap_or(rest.len());
                }
                _ if rest.starts_with("/*") => {
                    let end = rest[2..].find("*/").ok_or_else(|| {
                        Error::new(
                            ErrorKind::Syntax,
                            self.offset,
                            "Unterminated block comment.",
                        )
                    })? + 4;
                    line_break |= rest[..end].contains(is_line_terminator);
                    self.offset += end;
                }
                _ => return Ok(line_break),
            }
        }
    }

    fn digits(&mut self) {
        while self
            .source
            .as_bytes()
            .get(self.offset)
            .is_some_and(u8::is_ascii_digit)
        {
            self.offset += 1;
        }
    }

    fn number(&mut self) -> Result<f64, Error> {
        let start = self.offset;
        let bytes = self.source.as_bytes();
        if bytes[start] == b'0' {
            if matches!(
                bytes.get(start + 1),
                Some(b'x' | b'X' | b'o' | b'O' | b'b' | b'B')
            ) {
                return Err(Error::new(
                    ErrorKind::Unsupported,
                    start,
                    "Non-decimal literals are not implemented.",
                ));
            }
            if bytes.get(start + 1).is_some_and(u8::is_ascii_digit) {
                return Err(Error::new(
                    ErrorKind::Unsupported,
                    start,
                    "Legacy leading-zero literals are not implemented.",
                ));
            }
        }
        self.digits();
        if bytes.get(self.offset) == Some(&b'.') {
            self.offset += 1;
            self.digits();
        }
        if matches!(bytes.get(self.offset), Some(b'e' | b'E')) {
            self.offset += 1;
            if matches!(bytes.get(self.offset), Some(b'+' | b'-')) {
                self.offset += 1;
            }
            if !bytes.get(self.offset).is_some_and(u8::is_ascii_digit) {
                return Err(Error::new(
                    ErrorKind::Syntax,
                    self.offset,
                    "Expected an exponent digit.",
                ));
            }
            self.digits();
        }
        match bytes.get(self.offset) {
            Some(b'_' | b'n') => {
                return Err(Error::new(
                    ErrorKind::Unsupported,
                    self.offset,
                    "Numeric separators and BigInt literals are not implemented.",
                ));
            }
            Some(b'a'..=b'z' | b'A'..=b'Z' | b'$' | b'\\') => {
                return Err(Error::new(
                    ErrorKind::Syntax,
                    self.offset,
                    "An identifier cannot immediately follow a numeric literal.",
                ));
            }
            _ => {}
        }
        self.source[start..self.offset]
            .parse()
            .map_err(|_| Error::new(ErrorKind::Syntax, start, "Invalid decimal literal."))
    }
}

fn is_line_terminator(character: char) -> bool {
    matches!(character, '\n' | '\r' | '\u{2028}' | '\u{2029}')
}

fn is_whitespace(character: char) -> bool {
    // ECMA-262 WhiteSpace excludes Unicode NEL and includes BOM and all Space_Separator code points.
    matches!(
        character,
        '\t' | '\u{000b}' | '\u{000c}' | ' ' | '\u{00a0}' | '\u{1680}' | '\u{2000}'
            ..='\u{200a}' | '\u{202f}' | '\u{205f}' | '\u{3000}' | '\u{feff}'
    )
}
