use crate::{Error, ErrorKind};

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Kind {
    Number(f64),
    Identifier(String),
    Assign,
    Comma,
    OpenBrace,
    CloseBrace,
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
    pub escaped: bool,
}

#[derive(Clone)]
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
                escaped: false,
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
                escaped: false,
            });
        }
        if is_identifier_start(character) || character == '\\' {
            let (name, escaped) = self.identifier()?;
            return Ok(Token {
                kind: Kind::Identifier(name),
                offset,
                line_break_before,
                escaped,
            });
        }
        let rest = &self.source[offset..];
        if ["++", "--", "**", "+=", "-=", "*=", "/=", "%=", "==", "=>"]
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
            '=' => Kind::Assign,
            ',' => Kind::Comma,
            '{' => Kind::OpenBrace,
            '}' => Kind::CloseBrace,
            '@' => {
                return Err(Error::new(
                    ErrorKind::Syntax,
                    offset,
                    "Unexpected character.",
                ));
            }
            character if character.is_control() || !character.is_ascii() => {
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
                    "Syntax outside the implemented r8 subset.",
                ));
            }
        };
        Ok(Token {
            kind,
            offset,
            line_break_before,
            escaped: false,
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

    fn identifier(&mut self) -> Result<(String, bool), Error> {
        let mut name = String::new();
        let mut escaped = false;
        while let Some(character) = self.source[self.offset..].chars().next() {
            let valid = if name.is_empty() {
                is_identifier_start
            } else {
                is_identifier_part
            };
            let character = if character == '\\' {
                let offset = self.offset;
                let character = self.unicode_escape()?;
                if !valid(character) {
                    return Err(Error::new(
                        ErrorKind::Syntax,
                        offset,
                        "Invalid escaped identifier character.",
                    ));
                }
                escaped = true;
                character
            } else if valid(character) {
                self.offset += character.len_utf8();
                character
            } else {
                break;
            };
            name.push(character);
        }
        Ok((name, escaped))
    }

    fn unicode_escape(&mut self) -> Result<char, Error> {
        let start = self.offset;
        let invalid = || {
            Error::new(
                ErrorKind::Syntax,
                start,
                "Invalid Unicode escape in identifier.",
            )
        };
        if !self.source[start..].starts_with("\\u") {
            return Err(invalid());
        }
        self.offset += 2;
        let braced = self.source.as_bytes().get(self.offset) == Some(&b'{');
        if braced {
            self.offset += 1;
        }
        let mut value = 0_u32;
        let mut count = 0;
        loop {
            if braced && self.source.as_bytes().get(self.offset) == Some(&b'}') {
                self.offset += 1;
                break;
            }
            if !braced && count == 4 {
                break;
            }
            let digit = self.source[self.offset..]
                .chars()
                .next()
                .and_then(|character| character.to_digit(16))
                .ok_or_else(invalid)?;
            value = value
                .checked_mul(16)
                .and_then(|value| value.checked_add(digit))
                .filter(|value| *value <= 0x10ffff)
                .ok_or_else(invalid)?;
            self.offset += 1;
            count += 1;
        }
        if count == 0 {
            return Err(invalid());
        }
        char::from_u32(value).ok_or_else(invalid)
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
        if let Some(b'_' | b'n') = bytes.get(self.offset) {
            return Err(Error::new(
                ErrorKind::Unsupported,
                self.offset,
                "Numeric separators and BigInt literals are not implemented.",
            ));
        }
        if self.source[self.offset..]
            .chars()
            .next()
            .is_some_and(|character| is_identifier_start(character) || character == '\\')
        {
            return Err(Error::new(
                ErrorKind::Syntax,
                self.offset,
                "An identifier cannot immediately follow a numeric literal.",
            ));
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

fn is_identifier_start(character: char) -> bool {
    matches!(character, '$' | '_') || unicode_id_start::is_id_start(character)
}

fn is_identifier_part(character: char) -> bool {
    matches!(character, '$' | '_' | '\u{200c}' | '\u{200d}')
        || unicode_id_start::is_id_continue(character)
}
