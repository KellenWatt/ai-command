use std::fmt::{self, Display};

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Literal<'a> {
    Ident(&'a str),
    String(&'a str),
    Number(f64),
    Bool(bool),
    Nil, 
}
impl<'a> Display for Literal<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use Literal::*;
        match self {
            Ident(name) => write!(f, "{}", name),
            String(value) => write!(f, "\"{}\"", value),
            Number(value) => write!(f, "{}", value),
            Bool(value) => write!(f, "{}", value),
            Nil => write!(f, "nil")
        }
    }
}

impl<'a> Literal<'a> {
    pub fn to_owned(&self) -> OwnedLiteral {
        match self {
            Literal::Ident(s) => OwnedLiteral::Ident(s.to_string()),
            Literal::String(s) => OwnedLiteral::String(s.to_string()),
            Literal::Number(n) => OwnedLiteral::Number(*n),
            Literal::Bool(b) => OwnedLiteral::Bool(*b),
            Literal::Nil => OwnedLiteral::Nil,
        }
    }
}

#[derive(Clone, PartialEq, Debug)]
pub enum OwnedLiteral {
    Ident(String),
    String(String),
    Number(f64),
    Bool(bool),
    Nil,
}

impl Display for OwnedLiteral {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use OwnedLiteral::*;
        match self {
            Ident(name) => write!(f, "{}", name),
            String(value) => write!(f, "\"{}\"", value),
            Number(value) => write!(f, "{}", value),
            Bool(value) => write!(f, "{}", value),
            Nil => write!(f, "nil")
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
#[allow(dead_code)]
pub enum TokenType {
    // delimiters
    LeftBrace, RightBrace,
    Bar,
    // arithmetic
    LeftParen, RightParen,
    Minus, Plus, Slash, Star, Percent, Caret,
    MinusEqual, PlusEqual, SlashEqual, StarEqual, PercentEqual, CaretEqual,
    // Dollar, //?
    Semicolon,

    // comparison
    // Bang, 
    BangEqual,
    Equal, EqualEqual,
    Greater, GreaterEqual,
    Less, LessEqual,

    // Literals
    Word,
    Ident,
    Str,
    Num,
    Comment,

    // Keywords
    And, Or, Not, Xor,
    Group, Parallel, Race, Sequence,
    Deadline, // may not use, but reserve for later use
    False, True, Nil,
    While, Until,
    If, Else, Unless, // may not use unless, but reserving
    Use,
    // EOF,
}

impl Display for TokenType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Token<'a> {
    pub ty: TokenType,
    pub start: usize, // for manual line calculation - byte index
    pub len: usize, // in bytes, not graphemes
    pub lexeme: &'a str,
    pub literal: Option<Literal<'a>>,
}

impl<'a> Display for Token<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match &self.literal {
            Some(lit) => write!(f, "{} {} {}", self.ty, self.lexeme, lit),
            None => write!(f, "{} {}", self.ty, self.lexeme),
        }
    }
}

impl<'a> Token<'a> {
    pub fn to_owned(&self, source: &'a str) -> OwnedToken {
        let mut line = 1;
        for (i, b) in source.bytes().enumerate() {
            if i >= self.start {
                break;
            }
            if b == b'\n' {
                line += 1;
            }
        }

        OwnedToken {
            ty: self.ty,
            line,
            lexeme: self.lexeme.to_owned(),
            literal: self.literal.map(|l| l.to_owned()),
        }
    }

    pub fn artificial(ty: TokenType, lexeme: &str) -> Token<'_> {
        Token {
            ty,
            start: 0,
            len: 0,
            lexeme,
            literal: None,
        }
    }
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct OwnedToken {
    pub ty: TokenType,
    pub line: usize,
    pub lexeme: String,
    pub literal: Option<OwnedLiteral>,
}

