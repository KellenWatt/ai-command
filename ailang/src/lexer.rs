use std::collections::{VecDeque};

use unicode_segmentation::{UnicodeSegmentation, GraphemeIndices};

use crate::token::{Token, TokenType, Literal};
use crate::error::{Error};

// assumes input is a grapheme cluster
fn is_digit(g: &str) -> bool {
    let g = g.as_bytes();
    g[0] >= b'0' && g[0] <= b'9'
}

fn is_whitespace(g: &str) -> bool {
    g.chars().all(|c| c.is_whitespace())
}

fn is_reserved_char(g: &str) -> bool {
    is_whitespace(g) || (&["(", ")", "|", "{", "}", "-", "+", "*", "/", "%", "^", "!", "=", "<", ">", "#", "$", "\"", "'", ";"][..]).contains(&g)
}

fn check_for(word: &str, sord: &str, ty: TokenType) -> TokenType {
    if word == sord {ty} else {TokenType::Word}
}

fn word_type(word: &str) -> TokenType {
    let bord = word.as_bytes();
    match bord[0] {
        b'a' => check_for(word, "and", TokenType::And),
        b'o' => check_for(word, "or", TokenType::Or),
        b'n' => {
            if word == "not" {TokenType::Not}
            else if word == "nil" {TokenType::Nil}
            else {TokenType::Word}
        }
        b'g' => check_for(word, "group", TokenType::Group),
        b'p' => check_for(word, "parallel", TokenType::Parallel),
        b'r' => if let Some(b'e') = bord.get(1) {
            check_for(word, "return", TokenType::Return)
        } else {
            check_for(word, "race", TokenType::Race)
        }
        b'd' => check_for(word, "deadline", TokenType::Deadline),
        b'f' => check_for(word, "false", TokenType::False),
        b't' => check_for(word, "true", TokenType::True),
        b'w' => check_for(word, "while", TokenType::While),
        b'b' => check_for(word, "break", TokenType::Break),
        b'u' => {
            if word == "use" {TokenType::Use}
            else if word == "until" {TokenType::Until}
            else if word == "unless" {TokenType::Unless}
            else {TokenType::Word}
        }
        b'i' => check_for(word, "if", TokenType::If),
        b'e' => check_for(word, "else", TokenType::Else),
        b's' => check_for(word, "sequence", TokenType::Sequence),
        b'y' => check_for(word, "yield", TokenType::Yield),
        _ => TokenType::Word
    }
}

macro_rules! compound_op {
    ($self:expr, $ty:expr, $compty:expr) => {
        {
            let ty = if $self.matches("=") {$compty} else {$ty};
            $self.make_token(ty, None)
        }
    }
}

pub struct Lexer<'a> {
    source: &'a str, // keeping this around for debugging purposes
    chars: GraphemeIndices<'a>,
    peek_buf: VecDeque<(usize, &'a str)>,
    start: usize, // byte index
    current: usize, // byte length
    errors: Vec<Error>,

    peek_token: Option<Token<'a>>,
}

impl<'a> Lexer<'a> {
    pub fn new(source: &str) -> Lexer<'_> {
        Lexer {
            source: source,
            chars: source.grapheme_indices(true),
            peek_buf: VecDeque::with_capacity(1),
            start: 0,
            current: 0,
            errors: Vec::new(),
            peek_token: None,
        }
    }

    /// Note for future self: This function has nothing to do with lexing. This peeks a whole
    /// token. The other peek-y functions only do graphemes.
    pub fn peek(&mut self) -> Option<&Token<'a>> {
        if self.peek_token.is_none() {
            self.peek_token = self.next();
        }

        self.peek_token.as_ref()
    }

    pub fn source(&self) -> &'a str {
        self.source
    }

    #[allow(dead_code)]
    fn _scan(&mut self) -> Option<Token<'a>> {
        if self.peek_token.is_some() {
            return self.peek_token.take();
        }

        use TokenType::*;

        let hit_end = self.advance_while(|g| {
            // newline should be safe to skip since we're not line counting
            // Note: using the iterator here may be expensive. If so, refer to
            // https://www.unicode.org/Public/UCD/latest/ucd/PropList.txt
            // to homeroll some form of match statement
            g.chars().all(|c| c.is_whitespace())
        });
        if hit_end {return None;}
        self.pass();

        // self.start = self.current;
        let g = self.advance()?;

        match g {
            "(" => self.make_token(LeftParen, None),
            ")" => self.make_token(RightParen, None),
            "|" => self.make_token(Bar, None),
            "{" => self.make_token(LeftBrace, None),
            "}" => self.make_token(RightBrace, None),
            "-" => if is_digit(self._peek()?) {
                self.advance()?;
                self.number()
            } else {
                compound_op!(self, Minus, MinusEqual)
            }
            "+" => compound_op!(self, Plus, PlusEqual),
            "*" => compound_op!(self, Star, StarEqual),
            "/" => compound_op!(self, Slash, SlashEqual),
            "%" => compound_op!(self, Percent, PercentEqual),
            "^" => compound_op!(self, Caret, CaretEqual),
            "!" if self.matches("=") => self.make_token(BangEqual, None),
            "=" => compound_op!(self, Equal, EqualEqual),
            "<" => compound_op!(self, Less, LessEqual),
            ">" => compound_op!(self, Greater, GreaterEqual),
            "#" => {
                self.advance_while(|g| g != "\n");
                self.pass();
                return self._scan();
            }
            ";" => self.make_token(Semicolon, None),
            // single and double quotes are supported
            "\"" | "'" => self.string(g),
            _ if is_digit(g) => self.number(),
            
            "$" => {
                self.advance_while(|g| !is_reserved_char(g) && (g.chars().all(|c| {
                    c.is_alphabetic() || c.is_numeric() || c == '_'
                })));
                let name = &self.source[self.start+1..(self.current)];
                self.make_token(Ident, Some(Literal::Ident(name)))
            }
            _ => {
                self.advance_while(|g| !is_reserved_char(g));
                let word = &self.source[self.start..self.current];
                let ty = word_type(word);
                self.make_token(ty, None)
            }
        }
    }

    fn number(&mut self) -> Option<Token<'a>> {
        // don't care if hit end
        let _ = self.advance_while(|g| is_digit(g));
        if let Some(".") = self._peek() {
            // trailing "." after number is valid 
            let _ = self.advance();
            self.advance_while(|g| is_digit(g));
        }
        let num = self.source[self.start..(self.current)].parse().unwrap();
        self.make_token(TokenType::Num, Some(Literal::Number(num)))
    }

    fn string(&mut self, quote: &str) -> Option<Token<'a>> {
        // escaping is currently not supported. If it is necessary, that might get complicated
        let hit_end = self.advance_while(|g| g != quote);
        if hit_end {
            self.error("Unterminated string");
            return None;
        }

        self.advance(); // closing quote

        // the +/- 1 is perfectly safe, since those will always be quotes (even if expanded to
        // single or double quotes. Will never be Unicode).
        let value = &self.source[(self.start+1)..(self.current-1)];
        self.make_token(TokenType::Str, Some(Literal::String(value)))
    }

    // This can't fail. The Option is for ergonomics in scan
    fn make_token(&mut self, ty: TokenType, lit: Option<Literal<'a>>) -> Option<Token<'a>> {
        let text = &self.source[self.start..self.current];
        Some(Token {
            ty,
            start: self.start,
            len: text.len(),
            lexeme: text,
            literal: lit,
        })
    }

    fn _peek(&mut self) -> Option<&'a str> {
        self.peekn(1)
    }

    fn peekn(&mut self, n: usize) -> Option<&'a str> {
        if self.peek_buf.len() < n {
            for _ in self.peek_buf.len()..n {
                let (i, g) = self.chars.next()?;
                self.peek_buf.push_back((i, g))
            }
        }
        Some(self.peek_buf[n-1].1)
    }

    fn matches(&mut self, grapheme: &str) -> bool {
        if self._peek().map(|g| g == grapheme).unwrap_or(false) {
            let _ = self.advance();
            true
        } else {
            false
        }
    }

    fn advance(&mut self) -> Option<&'a str> {
        let (i, g) = if self.peek_buf.len() > 0 {
            unsafe {self.peek_buf.pop_front().unwrap_unchecked()}
        } else {
            self.chars.next()?
        };
        // self.last_width = g.as_bytes().len();
        self.current = i + g.as_bytes().len();
        Some(g)
    }

    fn advance_while<F: Fn(&str) -> bool>(&mut self, pred: F) -> bool {
        loop {
            let Some(g) = self._peek() else {
                return true;
            };
            if pred(g) {
                let _ = self.advance();
            } else {
                return false;
            }
        }
    }

    // Don't use this function at the beginning of `scan`. It would be confusing.
    fn pass(&mut self) {
        self.start = self.current;
    }

    fn error(&mut self, msg: &str) {
        let mut line = 1;
        for c in self.source.bytes() {
            if c == b'\n' {
                line += 1;
            }
        }
        self.errors.push(Error::Lex{
            line,
            msg: msg.to_string(),
        });
    } 

    pub fn errors(&self) -> &Vec<Error> {
        &self.errors
    }
}

impl<'a> Iterator for Lexer<'a> {
    type Item = Token<'a>;
    fn next(&mut self) -> Option<Token<'a>> {
        let tok = self._scan();
        if tok.as_ref().map(|t| t.ty == TokenType::Comment).unwrap_or(false) {
            self._scan()
        } else {
            tok
        }
    }
}
