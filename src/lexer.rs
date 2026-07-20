use std::fmt;

use crate::diag::{Diagnostic, Span};

#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    Fn,
    Namespace,
    End,
    If,
    Else,
    Elif,
    While,
    For,
    In,
    Break,
    Continue,
    Return,
    Var,
    Let,
    Print,
    Int,
    Float,
    Str,
    Bool,
    True,
    False,
    Goto,
    Struct,
    Ref,
    Out,
    Virtual,
    Domain,
    Own,
    Share,
    Joint,
    Whole,
    Exclusive,
    Move,
    Mut,
    Arrow, // ->
    Colon,
    Ident(String),
    IntLit(i64),
    FloatLit(f64),
    StrLit(String),
    LParen,
    RParen,
    LBracket,
    RBracket,
    LBrace,
    RBrace,
    Semicolon,
    Comma,
    Dot,
    DotDot, // ..
    Eq,
    EqEq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    Plus,
    PlusEq,
    Minus,
    MinusEq,
    Star,
    StarEq,
    Slash,
    SlashEq,
    Percent,
    PercentEq,
    Amp,
    AndAnd,
    Pipe,
    OrOr,
    Caret,
    Shl,
    Shr,
    Bang,
    Link(String),
    Eof,
}

impl Token {
    pub fn description(&self) -> String {
        match self {
            Token::Fn => "`fn`".into(),
            Token::Namespace => "`namespace`".into(),
            Token::End => "`end`".into(),
            Token::If => "`if`".into(),
            Token::Else => "`else`".into(),
            Token::Elif => "`elif`".into(),
            Token::While => "`while`".into(),
            Token::For => "`for`".into(),
            Token::In => "`in`".into(),
            Token::Break => "`break`".into(),
            Token::Continue => "`continue`".into(),
            Token::Return => "`return`".into(),
            Token::Var => "`var`".into(),
            Token::Let => "`let`".into(),
            Token::Print => "`print`".into(),
            Token::Int => "`int`".into(),
            Token::Float => "`float`".into(),
            Token::Str => "`str`".into(),
            Token::Bool => "`bool`".into(),
            Token::True => "`true`".into(),
            Token::False => "`false`".into(),
            Token::Goto => "`goto`".into(),
            Token::Struct => "`struct`".into(),
            Token::Ref => "`ref`".into(),
            Token::Out => "`out`".into(),
            Token::Virtual => "`virtual`".into(),
            Token::Domain => "`domain`".into(),
            Token::Own => "`own`".into(),
            Token::Share => "`share`".into(),
            Token::Joint => "`joint`".into(),
            Token::Whole => "`whole`".into(),
            Token::Exclusive => "`exclusive`".into(),
            Token::Move => "`move`".into(),
            Token::Mut => "`mut`".into(),
            Token::Arrow => "`->`".into(),
            Token::Colon => "`:`".into(),
            Token::Ident(s) => format!("`{s}`"),
            Token::IntLit(n) => format!("`{n}`"),
            Token::FloatLit(n) => format!("`{n}`"),
            Token::StrLit(s) => format!("string literal `\"{s}\"`"),
            Token::LParen => "`(`".into(),
            Token::RParen => "`)`".into(),
            Token::LBracket => "`[`".into(),
            Token::RBracket => "`]`".into(),
            Token::LBrace => "`{`".into(),
            Token::RBrace => "`}`".into(),
            Token::Semicolon => "`;`".into(),
            Token::Comma => "`,`".into(),
            Token::Dot => "`.`".into(),
            Token::DotDot => "`..`".into(),
            Token::Eq => "`=`".into(),
            Token::EqEq => "`==`".into(),
            Token::Ne => "`!=`".into(),
            Token::Lt => "`<`".into(),
            Token::Le => "`<=`".into(),
            Token::Gt => "`>`".into(),
            Token::Ge => "`>=`".into(),
            Token::Plus => "`+`".into(),
            Token::PlusEq => "`+=`".into(),
            Token::Minus => "`-`".into(),
            Token::MinusEq => "`-=`".into(),
            Token::Star => "`*`".into(),
            Token::StarEq => "`*=`".into(),
            Token::Slash => "`/`".into(),
            Token::SlashEq => "`/=`".into(),
            Token::Percent => "`%`".into(),
            Token::PercentEq => "`%=`".into(),
            Token::Amp => "`&`".into(),
            Token::AndAnd => "`&&`".into(),
            Token::Pipe => "`|`".into(),
            Token::OrOr => "`||`".into(),
            Token::Caret => "`^`".into(),
            Token::Shl => "`<<`".into(),
            Token::Shr => "`>>`".into(),
            Token::Bang => "`!`".into(),
            Token::Link(p) => format!("`!link {p}`"),
            Token::Eof => "end of file".into(),
        }
    }
}

impl fmt::Display for Token {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.description())
    }
}

#[derive(Debug, Clone)]
pub struct SpannedToken {
    pub kind: Token,
    pub span: Span,
}

pub struct Lexer<'a> {
    src: &'a [u8],
    pos: usize,
}

impl<'a> Lexer<'a> {
    pub fn new(src: &'a str) -> Self {
        let bytes = src.as_bytes();
        // Skip UTF-8 BOM if present
        let pos = if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
            3
        } else {
            0
        };
        Self { src: bytes, pos }
    }

    pub fn tokenize(mut self) -> Result<Vec<SpannedToken>, Diagnostic> {
        let mut tokens = Vec::new();
        loop {
            let tok = self.next_token()?;
            let is_eof = tok.kind == Token::Eof;
            tokens.push(tok);
            if is_eof {
                break;
            }
        }
        Ok(tokens)
    }

    fn peek(&self) -> Option<u8> {
        self.src.get(self.pos).copied()
    }

    fn starts_with_rest(&self, word: &[u8]) -> bool {
        self.src.get(self.pos..).is_some_and(|s| s.starts_with(word))
            && self
                .src
                .get(self.pos + word.len())
                .is_none_or(|c| !c.is_ascii_alphanumeric() && *c != b'_')
    }

    fn bump(&mut self) -> Option<u8> {
        let c = self.peek()?;
        self.pos += 1;
        Some(c)
    }

    fn span_from(&self, start: usize) -> Span {
        Span::new(start, self.pos)
    }

    fn err_at(&self, start: usize, msg: impl Into<String>, label: impl Into<String>) -> Diagnostic {
        Diagnostic::error(msg).label(self.span_from(start), label)
    }

    fn skip_ws_and_comments(&mut self) {
        loop {
            while let Some(c) = self.peek() {
                if c.is_ascii_whitespace() {
                    self.bump();
                } else {
                    break;
                }
            }
            if self.peek() == Some(b'/') && self.src.get(self.pos + 1) == Some(&b'/') {
                while let Some(c) = self.peek() {
                    if c == b'\n' {
                        break;
                    }
                    self.bump();
                }
                continue;
            }
            break;
        }
    }

    fn next_token(&mut self) -> Result<SpannedToken, Diagnostic> {
        self.skip_ws_and_comments();
        let start = self.pos;
        let Some(c) = self.peek() else {
            return Ok(SpannedToken {
                kind: Token::Eof,
                span: Span::new(start, start),
            });
        };

        let kind = match c {
            b'(' => {
                self.bump();
                Token::LParen
            }
            b')' => {
                self.bump();
                Token::RParen
            }
            b'[' => {
                self.bump();
                Token::LBracket
            }
            b']' => {
                self.bump();
                Token::RBracket
            }
            b'{' => {
                self.bump();
                Token::LBrace
            }
            b'}' => {
                self.bump();
                Token::RBrace
            }
            b';' => {
                self.bump();
                Token::Semicolon
            }
            b',' => {
                self.bump();
                Token::Comma
            }
            b'.' => {
                self.bump();
                if self.peek() == Some(b'.') {
                    self.bump();
                    Token::DotDot
                } else {
                    Token::Dot
                }
            }
            b'+' => {
                self.bump();
                if self.peek() == Some(b'=') {
                    self.bump();
                    Token::PlusEq
                } else {
                    Token::Plus
                }
            }
            b'-' => {
                self.bump();
                if self.peek() == Some(b'>') {
                    self.bump();
                    Token::Arrow
                } else if self.peek() == Some(b'=') {
                    self.bump();
                    Token::MinusEq
                } else {
                    Token::Minus
                }
            }
            b':' => {
                self.bump();
                Token::Colon
            }
            b'*' => {
                self.bump();
                if self.peek() == Some(b'=') {
                    self.bump();
                    Token::StarEq
                } else {
                    Token::Star
                }
            }
            b'/' => {
                self.bump();
                if self.peek() == Some(b'=') {
                    self.bump();
                    Token::SlashEq
                } else {
                    Token::Slash
                }
            }
            b'%' => {
                self.bump();
                if self.peek() == Some(b'=') {
                    self.bump();
                    Token::PercentEq
                } else {
                    Token::Percent
                }
            }
            b'=' => {
                self.bump();
                if self.peek() == Some(b'=') {
                    self.bump();
                    Token::EqEq
                } else {
                    Token::Eq
                }
            }
            b'!' => {
                self.bump();
                if self.peek() == Some(b'=') {
                    self.bump();
                    Token::Ne
                } else if self.starts_with_rest(b"link") {
                    for _ in 0..4 {
                        self.bump();
                    }
                    self.skip_ws_and_comments();
                    let path = self.read_path(start)?;
                    Token::Link(path)
                } else {
                    Token::Bang
                }
            }
            b'<' => {
                self.bump();
                if self.peek() == Some(b'=') {
                    self.bump();
                    Token::Le
                } else if self.peek() == Some(b'<') {
                    self.bump();
                    Token::Shl
                } else {
                    Token::Lt
                }
            }
            b'>' => {
                self.bump();
                if self.peek() == Some(b'=') {
                    self.bump();
                    Token::Ge
                } else if self.peek() == Some(b'>') {
                    self.bump();
                    Token::Shr
                } else {
                    Token::Gt
                }
            }
            b'&' => {
                self.bump();
                if self.peek() == Some(b'&') {
                    self.bump();
                    Token::AndAnd
                } else {
                    Token::Amp
                }
            }
            b'|' => {
                self.bump();
                if self.peek() == Some(b'|') {
                    self.bump();
                    Token::OrOr
                } else {
                    Token::Pipe
                }
            }
            b'^' => {
                self.bump();
                Token::Caret
            }
            b'"' => return self.read_string(start),
            b'0'..=b'9' => return self.read_number(start),
            b'a'..=b'z' | b'A'..=b'Z' | b'_' => {
                let ident = self.read_ident();
                keyword_or_ident(ident)
            }
            _ => {
                return Err(self.err_at(
                    start,
                    format!("unknown start of token: `{}`", c as char),
                    "unknown character",
                ));
            }
        };

        Ok(SpannedToken {
            kind,
            span: self.span_from(start),
        })
    }

    fn read_ident(&mut self) -> String {
        let start = self.pos;
        while let Some(c) = self.peek() {
            if c.is_ascii_alphanumeric() || c == b'_' {
                self.bump();
            } else {
                break;
            }
        }
        String::from_utf8_lossy(&self.src[start..self.pos]).into_owned()
    }

    fn read_path(&mut self, link_start: usize) -> Result<String, Diagnostic> {
        let start = self.pos;
        while let Some(c) = self.peek() {
            if c.is_ascii_alphanumeric() || c == b'_' || c == b'.' || c == b'/' || c == b'\\' {
                self.bump();
            } else {
                break;
            }
        }
        if start == self.pos {
            return Err(self.err_at(link_start, "expected path after `!link`", "missing path"));
        }
        Ok(String::from_utf8_lossy(&self.src[start..self.pos]).into_owned())
    }

    fn read_number(&mut self, start: usize) -> Result<SpannedToken, Diagnostic> {
        if self.src.get(start) == Some(&b'0') {
            if matches!(self.src.get(start + 1).map(|c| *c | 32), Some(b'x')) {
                self.pos = start + 2;
                let hex_start = self.pos;
                while let Some(c) = self.peek() {
                    if c.is_ascii_hexdigit() {
                        self.bump();
                    } else {
                        break;
                    }
                }
                if hex_start == self.pos {
                    return Err(self.err_at(
                        start,
                        "expected hex digits after `0x`",
                        "invalid hex integer literal",
                    ));
                }
                let s = String::from_utf8_lossy(&self.src[hex_start..self.pos]);
                let n: i64 = i64::from_str_radix(&s, 16).map_err(|_| {
                    self.err_at(start, format!("invalid hex integer literal `0x{s}`"), "not a valid `i64`")
                })?;
                return Ok(SpannedToken {
                    kind: Token::IntLit(n),
                    span: self.span_from(start),
                });
            }
        }

        self.pos = start;
        while let Some(c) = self.peek() {
            if c.is_ascii_digit() {
                self.bump();
            } else {
                break;
            }
        }
        let mut is_float = false;
        if self.peek() == Some(b'.') {
            if self.src.get(self.pos + 1).is_some_and(|c| c.is_ascii_digit()) {
                is_float = true;
                self.bump(); // '.'
                while let Some(c) = self.peek() {
                    if c.is_ascii_digit() {
                        self.bump();
                    } else {
                        break;
                    }
                }
            }
        }
        if matches!(self.peek(), Some(b'e' | b'E')) {
            is_float = true;
            self.bump();
            if matches!(self.peek(), Some(b'+' | b'-')) {
                self.bump();
            }
            let exp_start = self.pos;
            while let Some(c) = self.peek() {
                if c.is_ascii_digit() {
                    self.bump();
                } else {
                    break;
                }
            }
            if exp_start == self.pos {
                return Err(self.err_at(
                    start,
                    "expected digits in float exponent",
                    "invalid float literal",
                ));
            }
        }
        let s = String::from_utf8_lossy(&self.src[start..self.pos]);
        if is_float {
            let n: f64 = s.parse().map_err(|_| {
                self.err_at(start, format!("invalid float literal `{s}`"), "not a valid `f64`")
            })?;
            return Ok(SpannedToken {
                kind: Token::FloatLit(n),
                span: self.span_from(start),
            });
        }
        let n: i64 = s.parse().map_err(|_| {
            self.err_at(start, format!("invalid integer literal `{s}`"), "not a valid `i64`")
        })?;
        Ok(SpannedToken {
            kind: Token::IntLit(n),
            span: self.span_from(start),
        })
    }

    fn read_string(&mut self, start: usize) -> Result<SpannedToken, Diagnostic> {
        self.bump(); // opening "
        let mut out = String::new();
        loop {
            match self.bump() {
                None => {
                    return Err(self
                        .err_at(start, "unterminated string literal", "not closed here")
                        .help("add a closing `\"`"));
                }
                Some(b'"') => break,
                Some(b'\\') => match self.bump() {
                    Some(b'n') => out.push('\n'),
                    Some(b't') => out.push('\t'),
                    Some(b'r') => out.push('\r'),
                    Some(b'\\') => out.push('\\'),
                    Some(b'"') => out.push('"'),
                    Some(c) => out.push(c as char),
                    None => {
                        return Err(self.err_at(
                            start,
                            "unterminated string escape",
                            "string ends inside escape",
                        ));
                    }
                },
                Some(c) => out.push(c as char),
            }
        }
        Ok(SpannedToken {
            kind: Token::StrLit(out),
            span: self.span_from(start),
        })
    }
}

fn keyword_or_ident(s: String) -> Token {
    match s.as_str() {
        "fn" => Token::Fn,
        "namespace" => Token::Namespace,
        "end" => Token::End,
        "if" => Token::If,
        "else" => Token::Else,
        "elif" => Token::Elif,
        "while" => Token::While,
        "for" => Token::For,
        "in" => Token::In,
        "break" => Token::Break,
        "continue" => Token::Continue,
        "return" => Token::Return,
        "var" => Token::Var,
        "let" => Token::Let,
        "print" => Token::Print,
        "int" => Token::Int,
        "float" | "double" => Token::Float,
        "str" => Token::Str,
        "bool" => Token::Bool,
        "true" => Token::True,
        "false" => Token::False,
        "goto" => Token::Goto,
        "struct" => Token::Struct,
        "ref" => Token::Ref,
        "out" => Token::Out,
        "virtual" => Token::Virtual,
        "domain" => Token::Domain,
        "own" => Token::Own,
        "share" => Token::Share,
        "joint" => Token::Joint,
        "whole" => Token::Whole,
        "exclusive" => Token::Exclusive,
        "move" => Token::Move,
        "mut" => Token::Mut,
        _ => Token::Ident(s),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lex_hello() {
        let tokens = Lexer::new(r#"fn main() { console.writeline("hi"); }"#)
            .tokenize()
            .unwrap();
        assert!(matches!(tokens[0].kind, Token::Fn));
        assert!(matches!(tokens[1].kind, Token::Ident(ref s) if s == "main"));
    }
}
