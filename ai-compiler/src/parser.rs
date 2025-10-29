use crate::lexer::{Lexer};
use crate::token::{Token, TokenType, OwnedToken, Literal};
use crate::ast::{*};
use crate::error::{Error, Result};

// use std::collections::{VecDeque};

type ExprResult<'a> = Result<Box<Expr<'a>>>;
type StmtResult<'a> = Result<Stmt<'a>>;

pub struct Parser<'a> {
    tokens: Lexer<'a>,
    pub errors: Vec<Error>,
}

macro_rules! binary_expr {
    ($lt:lifetime, $name:ident, $next_level:ident, $kind:ident, [$($ty:ident),+]) => {
        fn $name(&mut self) -> ExprResult<'a> {
            // println!("{}", stringify!($name));
            let mut expr = self.$next_level()?;

            while $(self.check(TokenType::$ty))||+ {
                let op = self.advance();
                let right = self.$next_level()?;
                expr = Expr::$kind(expr, op, right);
            }

            Ok(expr)
        }
    }
}

macro_rules! default_error {
    ($self:expr, $msg:expr) => {
        let source = $self.tokens.source();
        let tok = $self.peek().map(|tok| tok.to_owned(source));
        $self.error(tok, $msg)?;
        unreachable!();
    }
}

#[allow(dead_code)]
impl<'a> Parser<'a> {
    pub fn new(lexer: Lexer<'a>) -> Parser<'a> {
        Parser {
            tokens: lexer,
            errors: Vec::new(),
        }
    }

    pub fn parse(&mut self) -> Option<Vec<Stmt<'a>>> {
        let mut statements = Vec::new();
        while !self.is_finished() {
            if let Some(stmt) = self.declaration() {
                statements.push(stmt);
            }
        }
        self.errors.extend_from_slice(self.tokens.errors());

        self.errors.is_empty().then_some(statements)
        // statements
    }

    fn declaration(&mut self) -> Option<Stmt<'a>> {
        // println!("declaration");
        use TokenType::*;
        let out = if self.check(Group) || self.check(Parallel) || self.check(Race) {
            self.group_declaration()
        } else if self.matches(Use) {
            self.use_statement()
        } else {
            self.statement()
        };
        match out {
            Ok(s) => Some(s),
            Err(e) => {
                self.errors.push(e);
                self.synchronize();
                None
            }
        }
    }

    fn use_statement(&mut self) -> StmtResult<'a> {
        // println!("use_statement");
        if !self.check(TokenType::Ident) {
            default_error!(self, "Expect identifier after 'use'");
        }
        let name = self.advance();

        let _ = self.consume(TokenType::Semicolon, "Expect ';' after 'use' statement")?;
        Ok(Stmt::r#use(name))
    }
    
    fn group_declaration(&mut self) -> StmtResult<'a> {
        let kind = self.advance();
        if kind.ty != TokenType::Group {
            let _ = self.consume(TokenType::Group, "Expect 'group' after parallel specifier in group declaration")?;
        }
        let name = self.consume(TokenType::Word, "Expect group name after keyword 'group'")?;

        let mut params = Vec::new();
        while !self.check(TokenType::LeftBrace) {
            if self.check(TokenType::Word) || self.check(TokenType::Ident) {
                params.push(self.advance());
            } else {
                default_error!(self, "Group parameters can only be identifiers or words");
            }
        }

        if kind.ty == TokenType::Group {
            let _ = self.consume(TokenType::LeftBrace, "Expect '{' after argument list")?;

            let mut body = Vec::new();
            while !self.check(TokenType::RightBrace) {
                let stmt = self.statement()?;
                body.push(stmt);
            }

            let _ = self.consume(TokenType::RightBrace, "Expect '}' after group declaration")?;
            
            Ok(Stmt::group(name, params, body))
        } else {
            let is_race = kind.ty == TokenType::Race;
            let body = self.parallel_block(is_race)?;

            Ok(Stmt::group(name, params, vec![body]))
        }
    }

    fn statement(&mut self) -> StmtResult<'a> {
        // println!("statement");
        use TokenType::*;
        if self.check(If) || self.check(Unless) {
            self.if_statement()
        } else if self.check(While) || self.check(Until) {
            self.while_statement()
        } else if self.check(Parallel) || self.check(Race) {
            self.parallel_statement()
        } else if self.check(Ident) {
            self.var_statement()
        } else if self.check(Word) {
            self.exec_statement()
        } else {
            default_error!(self, "Statements must be conditionals, loops, assignments, or calls");
        }
    }

    fn if_statement(&mut self) -> StmtResult<'a> {
        // println!("if_statement");
        let invert = self.advance().ty == TokenType::Unless;

        let condition = self.expression()?;

        let _ = self.consume(TokenType::LeftBrace, "Expect '{' after condition")?;
        let mut then_body = Vec::new();
        while !self.check(TokenType::RightBrace) {
            then_body.push(self.statement()?);
        }

        let _ = self.consume(TokenType::RightBrace, "Expect '}' after conditional body");
        
        let mut else_body = Vec::new();
        if self.matches(TokenType::Else) {
            if self.check(TokenType::If) || self.check(TokenType::Unless) {
                let stmt = self.if_statement()?;
                else_body.push(stmt);
            } else if self.matches(TokenType::LeftBrace) {
                while !self.check(TokenType::RightBrace) {
                    else_body.push(self.statement()?);
                }
                let _ = self.consume(TokenType::RightBrace, "Expect '}' after else body");
            } else {
                default_error!(self, "Expect '{' or another 'if' statement after 'else'");
            }
        }

        Ok(Stmt::r#if(condition, invert, then_body, else_body))
    }
    
    fn while_statement(&mut self) -> StmtResult<'a> {
        // println!("while_statement");
        let invert = self.advance().ty == TokenType::Until;

        let condition = self.expression()?;

        let _ = self.consume(TokenType::LeftBrace, "Expect '{' after condition")?;
        let mut body = Vec::new();
        while !self.check(TokenType::RightBrace) {
            body.push(self.statement()?);
        }

        let _ = self.consume(TokenType::RightBrace, "Expect '}' after loop body")?;

        Ok(Stmt::r#while(condition, invert, body))
    }
    
    fn var_statement(&mut self) -> StmtResult<'a> {
        use TokenType::*;
        // println!("var_statement");
        let name = self.advance();

        if self.peek().is_none() {
            self.error(None, "Expected assignment operator")?;
            unreachable!()
        }
        let op = self.advance();
        if op.ty == TokenType::Equal {

            let value = self.expression()?;
            let _ = self.consume(TokenType::Semicolon, "Expect ';' after assignment");
            
            return Ok(Stmt::var(name, value));
        }
        let binop = match op.ty {
            PlusEqual => (Plus, "+"),
            MinusEqual => (Minus, "-"),
            StarEqual => (Star, "*"),
            SlashEqual => (Slash, "/"),
            PercentEqual => (Percent, "%"),
            CaretEqual => (Caret, "^"),
            _ => {
                self.error(Some(op.to_owned(self.tokens.source())), "Expected assignment operator".into())?;
                unreachable!();
            }

        };

        let tok = Token::artificial(binop.0, binop.1);
        let var_expr = Expr::variable(name.clone());
        let expr = Expr::binary(var_expr, tok, self.expression()?);

        let _ = self.consume(TokenType::Semicolon, "Expect ';' after assignment");
        Ok(Stmt::var(name, expr))
    }
    
    fn exec_statement(&mut self) -> StmtResult<'a> {
        // println!("exec_statement");
        let name = self.advance();
        
        let mut args = Vec::new();
        while !self.check(TokenType::Semicolon) {
            if self.check(TokenType::Word) {
                args.push(self.advance().into());
            } else {
                let exp = self.expression()?;
                args.push(exp.into());
            }
        }
        let _ = self.consume(TokenType::Semicolon, "Expect ';' after call")?;

        Ok(Stmt::exec(name, args))
    }

    fn parallel_block(&mut self, is_race: bool) -> StmtResult<'a> {
        let _ = self.consume(TokenType::LeftBrace, "Expect '{' after 'parallel' keyword")?;

        let mut calls = Vec::new();
        while !self.check(TokenType::RightBrace) {
            let Stmt::Exec(call) = self.exec_statement()? else {unreachable!()};
            calls.push(call);
        }

        let _ = self.consume(TokenType::RightBrace, "Expect '}' after parallel statement")?;
    
        Ok(Stmt::parallel(calls, is_race))
    }

    fn parallel_statement(&mut self) -> StmtResult<'a> {
        let is_race = self.advance().ty == TokenType::Race;

        self.parallel_block(is_race)
    }

    fn expression(&mut self) -> ExprResult<'a> {
        // println!("expression");
        self.or()
    }

    binary_expr!('a, or, xor, logical, [Or]);
    binary_expr!('a, xor, and, logical, [Xor]);
    binary_expr!('a, and, equality, logical, [And]);
    binary_expr!('a, equality, comparison, binary, [BangEqual, EqualEqual]);
    binary_expr!('a, comparison, term, binary, [Greater, GreaterEqual, Less, LessEqual]);
    binary_expr!('a, term, factor, binary, [Minus, Plus]);
    binary_expr!('a, factor, exp, binary, [Slash, Star, Percent]);
    binary_expr!('a, exp, unary, binary, [Caret]);

    fn unary(&mut self) -> ExprResult<'a> {
        // println!("unary");
        if self.check(TokenType::Not) || self.check(TokenType::Minus) {
            let op = self.advance();
            let right = self.unary()?;
            Ok(Expr::unary(op, right))
        } else {
            self.primary()
        }
    }

    fn primary(&mut self) -> ExprResult<'a> {
        // println!("primary");
        use TokenType::*;
        Ok(if self.matches(True) {
            Expr::literal(Literal::Bool(true))
        } else if self.matches(False) {
            Expr::literal(Literal::Bool(false))
        } else if self.matches(Nil) {
            Expr::literal(Literal::Nil)
        } else if self.check(Num) || self.check(Str) {
            Expr::literal(self.advance().literal.unwrap())
        } else if self.check(Ident) {
            Expr::variable(self.advance())
        } else if self.matches(LeftParen) {
            let exp = self.expression()?;
            let _ = self.consume(RightParen, "Expect ')' after expression")?;
            Expr::grouping(exp, false)
        } else if self.matches(Bar) {
            let exp = self.expression()?;
            let _ = self.consume(Bar, "Expect closing '|' in absolute value expression")?;
            Expr::grouping(exp, true)
        } else {
            default_error!(self, "Expect expression");
        })
    }


    fn matches(&mut self, ty: TokenType) -> bool {
        if self.check(ty) {
            self.advance();
            return true;
        }
        false
    }

    fn matches_group(&mut self, tys: &[TokenType]) -> bool {
        for ty in tys {
            if self.check(*ty) {
                self.advance();
                return true;
            }
        }
        false
    }

    fn consume(&mut self, ty: TokenType, msg: &str) -> Result<Token<'a>> {
        if self.check(ty) {
            Ok(self.advance())
        } else {
            default_error!(self, msg);
        }
    }

    fn check(&mut self, ty: TokenType) -> bool {
        self.peek().map(|tok| tok.ty == ty).unwrap_or(false)
    }

    fn advance(&mut self) -> Token<'a> {
        let next = self.tokens.next().unwrap();
        next
    }

    fn is_finished(&mut self) -> bool {
        self.peek().is_none()
    }

    fn peek(&mut self) -> Option<&Token<'a>> {
        let out = self.tokens.peek();
        out
    }

    fn error(&mut self, tok: Option<OwnedToken>, msg: &str) -> Result<()> {
        Err(if let Some(tok) = tok {
            Error::Parse{tok, msg: msg.to_string()}
        } else {
            Error::EndOfStream{msg: msg.to_string()}
        })
    }

    fn synchronize(&mut self) {
        use TokenType::*;
        if let None = self.peek() {
            return;
        }
        let mut last_tok = self.advance();
        while let Some(tok) = self.peek() {
            if last_tok.ty == Semicolon {return;}
            match tok.ty {
                Use | Group | Race | Parallel | Sequence | If | Unless | While | Until => return,
                _ => {
                    last_tok = self.advance();
                }
            }
            
        }
    }
}


