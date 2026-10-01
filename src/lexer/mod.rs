pub mod token;

use crate::diag::Diagnostics;
use crate::span::Span;
use token::{Token, TokenKind};

pub struct Lexer<'a> {
    source: &'a str,
    chars: Vec<(usize, char)>,
    cursor: usize,
}

impl<'a> Lexer<'a> {
    pub fn new(source: &'a str) -> Self {
        let chars: Vec<(usize, char)> = source.char_indices().collect();
        Self {
            source,
            chars,
            cursor: 0,
        }
    }

    fn peek(&self) -> Option<char> {
        if self.cursor < self.chars.len() {
            Some(self.chars[self.cursor].1)
        } else {
            None
        }
    }

    fn peek_next(&self) -> Option<char> {
        if self.cursor + 1 < self.chars.len() {
            Some(self.chars[self.cursor + 1].1)
        } else {
            None
        }
    }

    fn advance(&mut self) -> Option<(usize, char)> {
        if self.cursor < self.chars.len() {
            let res = self.chars[self.cursor];
            self.cursor += 1;
            Some(res)
        } else {
            None
        }
    }

    fn current_pos(&self) -> usize {
        if self.cursor < self.chars.len() {
            self.chars[self.cursor].0
        } else {
            self.source.len()
        }
    }

    pub fn tokenize(&mut self, diag: &mut Diagnostics) -> Vec<Token> {
        let mut tokens = Vec::new();

        while let Some(c) = self.peek() {
            // Whitespace
            if c.is_whitespace() {
                self.advance();
                continue;
            }

            // Comments
            if c == '/' {
                if self.peek_next() == Some('/') {
                    // Line comment
                    self.advance();
                    self.advance();
                    while let Some(ch) = self.peek() {
                        self.advance();
                        if ch == '\n' {
                            break;
                        }
                    }
                    continue;
                } else if self.peek_next() == Some('*') {
                    // Block comment
                    let start = self.current_pos();
                    self.advance();
                    self.advance();
                    let mut closed = false;
                    while let Some(ch) = self.peek() {
                        if ch == '*' && self.peek_next() == Some('/') {
                            self.advance();
                            self.advance();
                            closed = true;
                            break;
                        }
                        self.advance();
                    }
                    if !closed {
                        diag.error(
                            Span::new(start, self.source.len()),
                            "Unterminated block comment",
                        );
                    }
                    continue;
                }
            }

            let start = self.current_pos();

            // Identifiers / Keywords
            if c.is_alphabetic() || c == '_' {
                let token = self.lex_ident_or_keyword(start);
                tokens.push(token);
                continue;
            }

            // Numbers
            if c.is_ascii_digit()
                || (c == '.' && self.peek_next().is_some_and(|n| n.is_ascii_digit()))
            {
                let token = self.lex_number(start, diag);
                tokens.push(token);
                continue;
            }

            // String literals
            if c == '"' {
                let token = self.lex_string(start, diag);
                tokens.push(token);
                continue;
            }

            // Character literals
            if c == '\'' {
                let token = self.lex_char(start, diag);
                tokens.push(token);
                continue;
            }

            // Operators & Delimiters
            let (pos, ch) = self.advance().unwrap();
            let kind = match ch {
                '+' => {
                    if self.peek() == Some('+') {
                        self.advance();
                        TokenKind::PlusPlus
                    } else if self.peek() == Some('=') {
                        self.advance();
                        TokenKind::PlusAssign
                    } else {
                        TokenKind::Plus
                    }
                }
                '-' => {
                    if self.peek() == Some('-') {
                        self.advance();
                        TokenKind::MinusMinus
                    } else if self.peek() == Some('>') {
                        self.advance();
                        TokenKind::Arrow
                    } else if self.peek() == Some('=') {
                        self.advance();
                        TokenKind::MinusAssign
                    } else {
                        TokenKind::Minus
                    }
                }
                '*' => {
                    if self.peek() == Some('=') {
                        self.advance();
                        TokenKind::StarAssign
                    } else {
                        TokenKind::Star
                    }
                }
                '/' => {
                    if self.peek() == Some('=') {
                        self.advance();
                        TokenKind::SlashAssign
                    } else {
                        TokenKind::Slash
                    }
                }
                '%' => {
                    if self.peek() == Some('=') {
                        self.advance();
                        TokenKind::PercentAssign
                    } else {
                        TokenKind::Percent
                    }
                }
                '&' => {
                    if self.peek() == Some('&') {
                        self.advance();
                        TokenKind::AmpAmp
                    } else if self.peek() == Some('=') {
                        self.advance();
                        TokenKind::AmpAssign
                    } else {
                        TokenKind::Amp
                    }
                }
                '|' => {
                    if self.peek() == Some('|') {
                        self.advance();
                        TokenKind::PipePipe
                    } else if self.peek() == Some('=') {
                        self.advance();
                        TokenKind::PipeAssign
                    } else {
                        TokenKind::Pipe
                    }
                }
                '^' => {
                    if self.peek() == Some('=') {
                        self.advance();
                        TokenKind::CaretAssign
                    } else {
                        TokenKind::Caret
                    }
                }
                '~' => TokenKind::Tilde,
                '!' => {
                    if self.peek() == Some('=') {
                        self.advance();
                        TokenKind::Ne
                    } else {
                        TokenKind::Exclaim
                    }
                }
                '=' => {
                    if self.peek() == Some('=') {
                        self.advance();
                        TokenKind::Eq
                    } else {
                        TokenKind::Assign
                    }
                }
                '<' => {
                    if self.peek() == Some('<') {
                        self.advance();
                        if self.peek() == Some('=') {
                            self.advance();
                            TokenKind::ShlAssign
                        } else {
                            TokenKind::Shl
                        }
                    } else if self.peek() == Some('=') {
                        self.advance();
                        TokenKind::Le
                    } else {
                        TokenKind::Lt
                    }
                }
                '>' => {
                    if self.peek() == Some('>') {
                        self.advance();
                        if self.peek() == Some('=') {
                            self.advance();
                            TokenKind::ShrAssign
                        } else {
                            TokenKind::Shr
                        }
                    } else if self.peek() == Some('=') {
                        self.advance();
                        TokenKind::Ge
                    } else {
                        TokenKind::Gt
                    }
                }
                '.' => {
                    if self.peek() == Some('.') && self.peek_next() == Some('.') {
                        self.advance();
                        self.advance();
                        TokenKind::Ellipsis
                    } else {
                        TokenKind::Dot
                    }
                }
                '?' => TokenKind::Question,
                ':' => TokenKind::Colon,
                ',' => TokenKind::Comma,
                ';' => TokenKind::Semicolon,
                '(' => TokenKind::LParen,
                ')' => TokenKind::RParen,
                '[' => TokenKind::LBracket,
                ']' => TokenKind::RBracket,
                '{' => TokenKind::LBrace,
                '}' => TokenKind::RBrace,
                '#' => {
                    if self.peek() == Some('#') {
                        self.advance();
                        TokenKind::HashHash
                    } else {
                        TokenKind::Hash
                    }
                }
                _ => {
                    diag.error(
                        Span::new(pos, pos + ch.len_utf8()),
                        format!("Unexpected character: '{}'", ch),
                    );
                    continue;
                }
            };

            let end = self.current_pos();
            tokens.push(Token::new(kind, Span::new(start, end)));
        }

        let eof_pos = self.source.len();
        tokens.push(Token::eof(Span::new(eof_pos, eof_pos)));
        tokens
    }

    fn lex_ident_or_keyword(&mut self, start: usize) -> Token {
        while let Some(c) = self.peek() {
            if c.is_alphanumeric() || c == '_' {
                self.advance();
            } else {
                break;
            }
        }
        let end = self.current_pos();
        let text = &self.source[start..end];

        let kind = match text {
            "return" => TokenKind::Return,
            "if" => TokenKind::If,
            "else" => TokenKind::Else,
            "while" => TokenKind::While,
            "for" => TokenKind::For,
            "do" => TokenKind::Do,
            "break" => TokenKind::Break,
            "continue" => TokenKind::Continue,
            "switch" => TokenKind::Switch,
            "case" => TokenKind::Case,
            "default" => TokenKind::Default,
            "goto" => TokenKind::Goto,
            "sizeof" => TokenKind::Sizeof,
            "_Alignof" | "alignof" => TokenKind::Alignof,
            "typedef" => TokenKind::Typedef,
            "struct" => TokenKind::Struct,
            "union" => TokenKind::Union,
            "enum" => TokenKind::Enum,
            "static" => TokenKind::Static,
            "extern" => TokenKind::Extern,
            "const" | "__const" | "__const__" => TokenKind::Const,
            "volatile" | "__volatile" | "__volatile__" => TokenKind::Volatile,
            "restrict" | "__restrict" | "__restrict__" => TokenKind::Restrict,
            "_Atomic" => TokenKind::Atomic,
            "__attribute__" | "__attribute" => TokenKind::Attribute,
            "__asm__" | "__asm" | "asm" => TokenKind::Asm,
            "__extension__" => TokenKind::Extension,
            "auto" => TokenKind::Auto,
            "register" => TokenKind::Register,
            "inline" | "__inline" | "__inline__" => TokenKind::Inline,
            "void" => TokenKind::Void,
            "_Bool" | "bool" => TokenKind::Bool,
            "char" => TokenKind::CharKw,
            "short" => TokenKind::Short,
            "int" => TokenKind::IntKw,
            "long" => TokenKind::Long,
            "signed" | "__signed" | "__signed__" => TokenKind::Signed,
            "unsigned" | "__unsigned" | "__unsigned__" => TokenKind::Unsigned,
            "float" => TokenKind::FloatKw,
            "double" => TokenKind::Double,
            _ => TokenKind::Ident(text.to_string()),
        };

        Token::new(kind, Span::new(start, end))
    }

    fn lex_number(&mut self, start: usize, diag: &mut Diagnostics) -> Token {
        let mut is_float = false;

        if self.peek() == Some('0') {
            if self.peek_next() == Some('x') || self.peek_next() == Some('X') {
                // Hexadecimal (integer or float)
                self.advance(); // '0'
                self.advance(); // 'x'
                let hex_start = self.current_pos();
                let mut is_hex_float = false;
                while let Some(c) = self.peek() {
                    if c.is_ascii_hexdigit() {
                        self.advance();
                    } else if c == '.' && !is_hex_float {
                        is_hex_float = true;
                        self.advance();
                    } else if c == 'p' || c == 'P' {
                        is_hex_float = true;
                        self.advance();
                        if self.peek() == Some('+') || self.peek() == Some('-') {
                            self.advance();
                        }
                        while let Some(exp_c) = self.peek() {
                            if exp_c.is_ascii_digit() {
                                self.advance();
                            } else {
                                break;
                            }
                        }
                        break;
                    } else {
                        break;
                    }
                }
                let hex_str = &self.source[start..self.current_pos()];
                if is_hex_float {
                    if self.peek() == Some('f')
                        || self.peek() == Some('F')
                        || self.peek() == Some('l')
                        || self.peek() == Some('L')
                    {
                        self.advance();
                    }
                    let val = parse_hex_float(hex_str);
                    return Token::new(TokenKind::Float(val), Span::new(start, self.current_pos()));
                } else {
                    let raw_hex = &self.source[hex_start..self.current_pos()];
                    self.consume_integer_suffixes();
                    let val = u64::from_str_radix(raw_hex, 16)
                        .map(|v| v as i64)
                        .unwrap_or(0);
                    return Token::new(TokenKind::Int(val), Span::new(start, self.current_pos()));
                }
            } else if self.peek_next() == Some('b') || self.peek_next() == Some('B') {
                // Binary
                self.advance(); // '0'
                self.advance(); // 'b'
                let bin_start = self.current_pos();
                while let Some(c) = self.peek() {
                    if c == '0' || c == '1' {
                        self.advance();
                    } else {
                        break;
                    }
                }
                let bin_str = &self.source[bin_start..self.current_pos()];
                self.consume_integer_suffixes();
                let val = u64::from_str_radix(bin_str, 2)
                    .map(|v| v as i64)
                    .unwrap_or(0);
                return Token::new(TokenKind::Int(val), Span::new(start, self.current_pos()));
            }
        }

        let mut has_dot = false;
        let mut has_exp = false;

        while let Some(c) = self.peek() {
            if c.is_ascii_digit() {
                self.advance();
            } else if c == '.' && !has_dot && !has_exp && self.peek_next() != Some('.') {
                has_dot = true;
                is_float = true;
                self.advance();
            } else if (c == 'e' || c == 'E') && !has_exp {
                has_exp = true;
                is_float = true;
                self.advance();
                if self.peek() == Some('+') || self.peek() == Some('-') {
                    self.advance();
                }
                while let Some(exp_d) = self.peek() {
                    if exp_d.is_ascii_digit() {
                        self.advance();
                    } else {
                        break;
                    }
                }
            } else {
                break;
            }
        }

        let num_end = self.current_pos();
        let num_str = &self.source[start..num_end];

        if is_float {
            if self.peek() == Some('f')
                || self.peek() == Some('F')
                || self.peek() == Some('l')
                || self.peek() == Some('L')
            {
                self.advance();
            }
            let val: f64 = num_str.parse().unwrap_or_else(|_| {
                diag.error(Span::new(start, num_end), "Invalid float literal");
                0.0
            });
            Token::new(TokenKind::Float(val), Span::new(start, self.current_pos()))
        } else {
            self.consume_integer_suffixes();
            let val: i64 = if num_str.starts_with('0')
                && num_str.len() > 1
                && num_str.chars().all(|c| ('0'..='7').contains(&c))
            {
                // Octal
                u64::from_str_radix(num_str, 8)
                    .map(|v| v as i64)
                    .unwrap_or(0)
            } else {
                num_str
                    .parse::<i64>()
                    .or_else(|_| num_str.parse::<u64>().map(|v| v as i64))
                    .unwrap_or_else(|_| {
                        diag.error(
                            Span::new(start, num_end),
                            format!("Invalid integer literal '{}'", num_str),
                        );
                        0
                    })
            };
            Token::new(TokenKind::Int(val), Span::new(start, self.current_pos()))
        }
    }

    fn consume_integer_suffixes(&mut self) {
        while let Some(c) = self.peek() {
            if c == 'u' || c == 'U' || c == 'l' || c == 'L' {
                self.advance();
            } else {
                break;
            }
        }
    }

    fn lex_char(&mut self, start: usize, diag: &mut Diagnostics) -> Token {
        self.advance(); // Consume opening '\''
        let val = if let Some(c) = self.peek() {
            if c == '\\' {
                self.advance();
                self.lex_escape(diag)
            } else if c == '\'' {
                diag.error(
                    Span::new(start, self.current_pos() + 1),
                    "Empty character constant",
                );
                '\0'
            } else {
                self.advance();
                c
            }
        } else {
            diag.error(
                Span::new(start, self.source.len()),
                "Unterminated character literal",
            );
            '\0'
        };

        if self.peek() == Some('\'') {
            self.advance();
        } else {
            diag.error(
                Span::new(start, self.current_pos()),
                "Unclosed character constant",
            );
        }

        Token::new(TokenKind::Char(val), Span::new(start, self.current_pos()))
    }

    fn lex_string(&mut self, start: usize, diag: &mut Diagnostics) -> Token {
        self.advance(); // Consume opening '"'
        let mut buf = String::new();

        while let Some(c) = self.peek() {
            if c == '"' {
                self.advance();
                return Token::new(TokenKind::String(buf), Span::new(start, self.current_pos()));
            } else if c == '\\' {
                self.advance();
                let esc = self.lex_escape(diag);
                buf.push(esc);
            } else if c == '\n' {
                diag.error(
                    Span::new(start, self.current_pos()),
                    "Unterminated string literal",
                );
                break;
            } else {
                self.advance();
                buf.push(c);
            }
        }

        diag.error(
            Span::new(start, self.source.len()),
            "Unterminated string literal",
        );
        Token::new(TokenKind::String(buf), Span::new(start, self.current_pos()))
    }

    fn lex_escape(&mut self, diag: &mut Diagnostics) -> char {
        match self.advance() {
            Some((_, 'n')) => '\n',
            Some((_, 't')) => '\t',
            Some((_, 'r')) => '\r',
            Some((_, '\\')) => '\\',
            Some((_, '\'')) => '\'',
            Some((_, '"')) => '"',
            Some((_, '?')) => '?',
            Some((_, 'a')) => '\x07',
            Some((_, 'b')) => '\x08',
            Some((_, 'f')) => '\x0c',
            Some((_, 'v')) => '\x0b',
            Some((_, ch @ '0'..='7')) => {
                let mut val = ch.to_digit(8).unwrap();
                let mut count = 1;
                while count < 3 {
                    if let Some(c) = self.peek() {
                        if let Some(d) = c.to_digit(8) {
                            self.advance();
                            val = val * 8 + d;
                            count += 1;
                        } else {
                            break;
                        }
                    } else {
                        break;
                    }
                }
                char::from_u32(val).unwrap_or('\0')
            }
            Some((pos, 'x')) => {
                let mut val = 0u32;
                let mut count = 0;
                while let Some(c) = self.peek() {
                    if let Some(d) = c.to_digit(16) {
                        self.advance();
                        val = val * 16 + d;
                        count += 1;
                        if count == 2 {
                            break;
                        }
                    } else {
                        break;
                    }
                }
                if count == 0 {
                    diag.error(
                        Span::new(pos, pos + 2),
                        "\\x used with no following hex digits",
                    );
                }
                char::from_u32(val).unwrap_or('\0')
            }
            Some((pos, ch)) => {
                diag.warning(
                    Span::new(pos, pos + ch.len_utf8()),
                    format!("Unknown escape sequence: \\{}", ch),
                );
                ch
            }
            None => {
                diag.error(
                    Span::new(self.source.len(), self.source.len()),
                    "Expected escape character",
                );
                '\0'
            }
        }
    }
}

fn parse_hex_float(s: &str) -> f64 {
    let s = s.trim_start_matches("0x").trim_start_matches("0X");
    let (mantissa_str, exp_val) = if let Some((m, e)) = s.split_once(['p', 'P']) {
        let exp: i32 = e.parse().unwrap_or(0);
        (m, exp)
    } else {
        (s, 0)
    };

    let (int_part, frac_part) = if let Some((i, f)) = mantissa_str.split_once('.') {
        (i, f)
    } else {
        (mantissa_str, "")
    };

    let mut val = 0.0f64;
    if !int_part.is_empty()
        && let Ok(i) = u64::from_str_radix(int_part, 16)
    {
        val += i as f64;
    }

    let mut scale = 1.0f64 / 16.0f64;
    for ch in frac_part.chars() {
        if let Some(d) = ch.to_digit(16) {
            val += (d as f64) * scale;
            scale /= 16.0;
        }
    }

    if exp_val >= 0 {
        val * (2.0f64).powi(exp_val)
    } else {
        val / (2.0f64).powi(-exp_val)
    }
}
