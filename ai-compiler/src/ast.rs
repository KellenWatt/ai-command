#![allow(dead_code)]
use pastey::paste;
use crate::token::{Literal as LexLiteral, Token};


macro_rules! expr {
    ($($ty: ident),*$(,)?) => {
        #[allow(dead_code)]
        pub enum Expr<'a> {
            $($ty($ty<'a>)),*
        }

        #[allow(dead_code)]
        impl<'a> Expr<'a> {
            pub fn accept<R>(&self, visitor: &impl ExprVisitor<'a, R>) -> R {
                paste! {
                match self {
                    $(Expr::$ty(e) => visitor.[<visit_ $ty:lower _expr>](e)),+
                }
                }
            }
            
            pub fn accept_mut<R>(&self, visitor: &mut impl ExprVisitorMut<'a, R>) -> R {
                paste! {
                match self {
                    $(Expr::$ty(e) => visitor.[<visit_ $ty:lower _expr>](e)),+
                }
                }
            }
        }

        paste! {
            #[allow(dead_code)]
            pub trait ExprVisitor<'a, R> {
                $(fn [< visit_ $ty:lower _expr >](&self, expr: &$ty<'a>) -> R;)*
            }
            
            #[allow(dead_code)]
            pub trait ExprVisitorMut<'a, R> {
                $(fn [< visit_ $ty:lower _expr >](&mut self, expr: &$ty<'a>) -> R;)*
            }
        }
    }
}

expr! {
    Binary,
    Grouping,
    Literal,
    Logical,
    Unary,
    Variable,
}

impl<'a> Expr<'a> {
    pub fn binary(left: Box<Expr<'a>>, op: Token<'a>, right: Box<Expr<'a>>) -> Box<Expr<'a>> {
        Box::new(Expr::Binary(Binary {left, op, right}))
    }

    pub fn grouping(expression: Box<Expr<'a>>, abs: bool) -> Box<Expr<'a>> {
        Box::new(Expr::Grouping(Grouping {expression, abs}))
    }

    pub fn literal(value: LexLiteral<'a>) -> Box<Expr<'a>> {
        Box::new(Expr::Literal(Literal {value}))
    }

    pub fn logical(left: Box<Expr<'a>>, op: Token<'a>, right: Box<Expr<'a>>) -> Box<Expr<'a>> {
        Box::new(Expr::Logical(Logical {left, op, right}))
    }

    pub fn unary(op: Token<'a>, right: Box<Expr<'a>>) -> Box<Expr<'a>> {
        Box::new(Expr::Unary(Unary {op, right}))
    }

    pub fn variable(name: Token<'a>) -> Box<Expr<'a>> {
        Box::new(Expr::Variable(Variable {name}))
    }
}

pub struct Binary<'a> {
    pub left: Box<Expr<'a>>,
    pub op: Token<'a>,
    pub right: Box<Expr<'a>>
}

pub struct Grouping<'a> {
    pub expression: Box<Expr<'a>>,
    pub abs: bool,
}

pub struct Literal<'a> {
    pub value: LexLiteral<'a>,
}

pub struct Logical<'a> {
    pub left: Box<Expr<'a>>,
    pub op: Token<'a>,
    pub right: Box<Expr<'a>>,
}

pub struct Unary<'a> {
    pub op: Token<'a>, 
    pub right: Box<Expr<'a>>,
}

pub struct Variable<'a> {
    pub name: Token<'a>,
}

macro_rules! stmt {
    ($($ty: ident),*$(,)?) => {
        #[allow(dead_code)]
        pub enum Stmt<'a> {
            $($ty($ty<'a>)),*
        }

        #[allow(dead_code)]
        impl<'a> Stmt<'a> {
            pub fn accept<R>(&self, visitor: &impl StmtVisitor<'a, R>) -> R {
                paste! {
                match self {
                    $(Stmt::$ty(e) => visitor.[<visit_ $ty:lower _stmt>](e)),+
                }
                }
            }
            
            pub fn accept_mut<R>(&self, visitor: &mut impl StmtVisitorMut<'a, R>) -> R {
                paste! {
                match self {
                    $(Stmt::$ty(e) => visitor.[<visit_ $ty:lower _stmt>](e)),+
                }
                }
            }
        }

        paste! {
            #[allow(dead_code)]
            pub trait StmtVisitor<'a, R> {
                $(fn [< visit_ $ty:lower _stmt >](&self, stmt: &$ty<'a>) -> R;)*
            }
            
            #[allow(dead_code)]
            pub trait StmtVisitorMut<'a, R> {
                $(fn [< visit_ $ty:lower _stmt >](&mut self, stmt: &$ty<'a>) -> R;)*
            }
        }
    }
}

stmt! {
    Group,
    Use,
    If,
    While,
    Exec,
    Parallel,
    Var,
    Return,
    Yield,
    Break,
}

impl<'a> Stmt<'a> {
    pub fn group(name: Token<'a>, params: Vec<Token<'a>>, statements: Vec<Stmt<'a>>) -> Stmt<'a> {
        Stmt::Group(Group {name, params, statements})
    }
    pub fn r#use(name: Token<'a>) -> Stmt<'a> {
        Stmt::Use(Use{name})
    }
    pub fn r#if(condition: Box<Expr<'a>>, invert: bool, then_branch: Vec<Stmt<'a>>, else_branch: Vec<Stmt<'a>>) -> Stmt<'a> {
        Stmt::If(If{condition, invert, then_branch, else_branch})
    }
    pub fn r#while(condition: Box<Expr<'a>>, invert: bool, body: Vec<Stmt<'a>>) -> Stmt<'a> {
        Stmt::While(While{condition, invert, body})
    }
    pub fn exec(name: Token<'a>, args: Vec<Arg<'a>>) -> Stmt<'a> {
        Stmt::Exec(Exec{name, args})
    }
    pub fn parallel(calls: Vec<Exec<'a>>, race: bool) -> Stmt<'a> {
        Stmt::Parallel(Parallel{calls, race})
    }
    pub fn var(name: Token<'a>, value: Box<Expr<'a>>) -> Stmt<'a> {
        Stmt::Var(Var{name, value})
    }
    pub fn r#return(tok: Token<'a>) -> Stmt<'a> {
        Stmt::Return(Return{tok})
    }
    pub fn r#yield(tok: Token<'a>) -> Stmt<'a> {
        Stmt::Yield(Yield{tok})
    }
    pub fn r#break(tok: Token<'a>) -> Stmt<'a> {
        Stmt::Break(Break{tok})
    }
}

#[derive(PartialEq, Clone, Copy, Debug)]
pub enum GroupKind {
    Sequence,
    Parallel,
    Race,
}

pub struct Group<'a> {
    // pub kind: GroupKind,
    pub name: Token<'a>,
    pub params: Vec<Token<'a>>,
    pub statements: Vec<Stmt<'a>>,
}

pub struct Use<'a> {
    pub name: Token<'a>,
}

pub struct If<'a> {
    pub condition: Box<Expr<'a>>,
    pub invert: bool,
    pub then_branch: Vec<Stmt<'a>>,
    pub else_branch: Vec<Stmt<'a>>, 
    // An "else if" is modeled as a Stmt::If that's the only member of else_branch
    // Lacking an "else" is simply modeled as an empty else_branch
}

pub struct While<'a> {
    pub condition: Box<Expr<'a>>,
    pub invert: bool,
    pub body: Vec<Stmt<'a>>,
}

pub enum Arg<'a> {
    Word(Token<'a>),
    Value(Box<Expr<'a>>),
}

impl<'a> From<Token<'a>> for Arg<'a> {
    fn from(value: Token<'a>) -> Arg<'a> {
        Arg::Word(value)
    }
}

impl<'a> From<Box<Expr<'a>>> for Arg<'a> {
    fn from(value: Box<Expr<'a>>) -> Arg<'a> {
        Arg::Value(value)
    }
}

pub struct Exec<'a> {
    pub name: Token<'a>,
    pub args: Vec<Arg<'a>>
}

pub struct Parallel<'a> {
    pub calls: Vec<Exec<'a>>,
    pub race: bool,
}

pub struct Var<'a> {
    pub name: Token<'a>,
    pub value: Box<Expr<'a>>,
}

pub struct Return<'a> {
    pub tok: Token<'a>,
}

pub struct Yield<'a> {
    pub tok: Token<'a>,
}

pub struct Break<'a> {
    pub tok: Token<'a>,
}
