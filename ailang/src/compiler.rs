#![allow(dead_code)]
use std::fmt::{Display, Formatter, Result as FmtResult};
use std::collections::{HashMap, HashSet};
use unicode_segmentation::{UnicodeSegmentation};
use itertools::Itertools;

use std::sync::atomic::{AtomicBool, Ordering};

use crate::token::{TokenType};
use crate::token::{Literal as LexLiteral};
use crate::ast::{Arg as AstArg, *};
use crate::error::Error;

#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Number(f64),
    String(String),
    Bool(bool),
    Nil,
}

impl Value {
    pub fn truthy(&self) -> bool {
        match self {
            Value::Number(n) => *n != 0.0,
            Value::String(s) => s.len() > 0,
            Value::Bool(b) => *b,
            Value::Nil => false,
        }
    }
    
    pub fn is_num(&self) -> bool {
        if let Value::Number(_) = self {true} else {false}
    }

    pub fn is_str(&self) -> bool {
        if let Value::String(_) = self {true} else {false}
    }
}

impl Display for Value {
    fn fmt(&self, f: &mut Formatter) -> FmtResult {
        match self {
            Value::Number(n) => write!(f, "{}", n),
            Value::String(s) => write!(f, "\"{}\"", s),
            Value::Bool(b) => write!(f, "{}", b),
            Value::Nil => write!(f, "nil"),
        }
    }
}

impl std::str::FromStr for Value {
    type Err = Error;
    fn from_str(value: &str) -> Result<Value, Error> {
        if value.as_bytes()[0] == b'"' {
            Ok(Value::String(value.to_string()))
        } else if value.as_bytes()[0].is_ascii_digit() {
            value.parse().map(|n| Value::Number(n)).map_err(|_| Error::IRParse {
                line: 0,
                msg: "Invalid word, expected number".into(),
            })
        } else if value == "true" || value == "false" {
            Ok(Value::Bool(value == "true"))
        } else if value == "nil" {
            Ok(Value::Nil)
        } else {
            Err(Error::IRParse {
                line: 0,
                msg: format!("Unrecognized value: '{}'", value),
            })
        }
    }
}

#[derive(Clone)]
pub enum Op {
    // Use(String),

    Load(usize), // relative index of the variable on the stack
    Store(usize),
    Get(String),
    Set(String),
    Push(Value),
    Pop,
    Dup,

    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Exp,
    Neg,
    Abs,

    And,
    Or,
    Not,
    Xor,

    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,

    Jump(isize), // absolute position
    JumpUnless(isize),
    JumpIf(isize),

    Label(String),
    Call(String, usize),
    CallParallel(Vec<(String, usize)>), // first usize is reverse offset. Will always be reverse
    CallRace(Vec<(String, usize)>),
    // StartPara(usize, usize), // call count, total arg count
    Yield,
    Return,
}

impl Op {
    pub fn is_call(&self) -> bool {
        match self {
            Op::Call(_, _) | Op::CallParallel(_) | Op::CallRace(_) => true,
            _ => false,
        }
    }
}

fn format_calls(calls: &[(String, usize)]) -> String {
    let mut out = String::new();
    for (name, arity) in calls {
        out += &format!(" \"{}\" {}", name, arity);
    }
    out
}

impl Display for Op {
    fn fmt(&self, f: &mut Formatter) -> FmtResult {
        use Op::*;
        match self {
            // Use(s) => write!(f, "use \"{}\"", s),
            Load(a) => write!(f, "load {}", a),
            Store(a) => write!(f, "store {}", a),
            Get(s) => write!(f, "get \"{}\"", s),
            Set(s) => write!(f, "get \"{}\"", s),
            Push(v) => write!(f, "push {}", v),
            Jump(a) => write!(f, "jump {}", a),
            JumpUnless(a) => write!(f, "jump_unless {}", a),
            JumpIf(a) => write!(f, "jump_if {}", a),
            Label(s) => write!(f, "{}:", s),
            Call(s, p) => write!(f, "call \"{}\" {}", s, p),
            // StartPara(n, m) => write!(f, "start_para {} {}", n, m),
            CallParallel(calls) => write!(f, "call_parallel{}", format_calls(calls)),
            CallRace(calls) => write!(f, "call_race{}", format_calls(calls)),
            Return => write!(f, "return"),
            Yield => write!(f, "yield"),
            Pop => write!(f, "pop"),
            Dup => write!(f, "dup"),
            Add => write!(f, "add"),
            Sub => write!(f, "sub"),
            Mul => write!(f, "mul"),
            Div => write!(f, "div"),
            Mod => write!(f, "mod"),
            Exp => write!(f, "exp"),
            Neg => write!(f, "neg"),
            Abs => write!(f, "abs"),
            And => write!(f, "and"),
            Or => write!(f, "or"),
            Not => write!(f, "not"),
            Xor => write!(f, "xor"),
            Eq => write!(f, "eq"),
            Ne => write!(f, "ne"),
            Lt => write!(f, "lt"),
            Le => write!(f, "le"),
            Gt => write!(f, "gt"),
            Ge => write!(f, "ge"),
        }
    }
}

fn shell_split(input: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut start = 0;
    let mut quoted = false;
    let mut last_space = false;
    
    for (i, g) in input.grapheme_indices(true) {
        if last_space {
            last_space = false;
            start += 1;
            continue;
        }
        if g == "\"" {
            quoted = !quoted;
            continue;
        }
        if g == " " && !quoted {
            last_space = true;
            parts.push(&input[start..i]);
            start = i+1;
        }
    }

    parts.push(&input[start..]);

    parts
}

fn parse_parallel_args(input: &[&str]) -> Result<Vec<(String, usize)>, Error> {
    let (calls, rest) = input.as_chunks::<2>();
    if rest.len()  != 0 {
        return Err(Error::IRParse{line: 0, msg: format!("Every call in parallel call requires an arity")});
    }
    
    let mut calls_out = Vec::new();
    for call in calls {
        if call[0].as_bytes()[0]  != b'"' {
            return Err(Error::IRParse{line: 0, msg: "parallel calls must take a string".into()});
        } else if !call[0][1..].contains("\"") {
            return Err(Error::IRParse{line: 0, msg: "Unterminated string in parallel_call".into()});
        } else {
            let name = call[0][1..call[0].len()-1].to_string();

            let arity = call[1].parse().map_err(|_| Error::IRParse {
                line: 0,
                msg: "Calls require the arity as a second argument".into(),
            })?;
            calls_out.push((name, arity));
        }
    }
    Ok(calls_out)
}

macro_rules! expect_len {
    ($list:expr, $index:expr, $name:expr) => {
        {
            $list.get($index).ok_or(Error::IRParse {
                line: 0,
                msg: format!("'{}' expects {} arguments", $name, $index),
            })?
        }
    }
}


macro_rules! parse_string {
    ($value:expr, $op:expr) => {
        {
            if !$value.starts_with("\"") {
                Err(Error::IRParse{line: 0, msg: format!("'{}' must take a string", $op)})
            } else if $value.len() < 2 || !$value.ends_with("\"") {
                Err(Error::IRParse{line: 0, msg: format!("Unterminated string in '{}'", $op)})
            } else {
                Ok($value[1..$value.len()-1].to_string())
            }
        }
    }
}

impl std::str::FromStr for Op {
    type Err = Error;
    fn from_str(value: &str) -> Result<Op, Error> {
        // let parts = value.split(" ").collect::<Vec<&str>>();
        let parts = shell_split(value);
        if parts.len() == 0 {
            return Err(Error::IRParse {
                line: 0,
                msg: "No instruction present".into(),
            })
        }
        match parts[0] {
            "load" => expect_len!(parts, 1, "load").parse().map(|a| Op::Load(a)).map_err(|_| Error::IRParse {
                line: 0,
                msg: format!("Invalid stack address: '{}'", parts[1]),
            }),
            "store" => expect_len!(parts, 1, "store").parse().map(|a| Op::Store(a)).map_err(|_| Error::IRParse {
                line: 0,
                msg: format!("Invalid stack address: '{}'", parts[1]),
            }),
            "get" => Ok(Op::Get(parse_string!(expect_len!(parts, 1, "get"), "get")?)),
            "set" => Ok(Op::Set(parse_string!(expect_len!(parts, 1, "set"), "set")?)),
            "push" => expect_len!(parts, 1, "push").parse().map(|v| Op::Push(v)),
            "jump" => expect_len!(parts, 1, "jump").parse().map(|a| Op::Jump(a)).map_err(|_| Error::IRParse {
                line: 0,
                msg: format!("Invalid stack address: '{}'", parts[1]),
            }),
            "jump_unless" => expect_len!(parts, 1, "jump_unless").parse().map(|a| Op::JumpUnless(a)).map_err(|_| Error::IRParse {
                line: 0,
                msg: format!("Invalid stack address: '{}'", parts[1]),
            }),
            "jump_if" => expect_len!(parts, 1, "jump_if").parse().map(|a| Op::JumpIf(a)).map_err(|_| Error::IRParse {
                line: 0,
                msg: format!("Invalid stack address: '{}'", parts[1]),
            }),
            "call" => {
                let arity = expect_len!(parts, 2, "call").parse().map_err(|_| Error::IRParse {
                    line: 0,
                    msg: "Calls require the arity as a second argument".into(),
                })?;
                let name = parse_string!(parts[1], "call")?;
                Ok(Op::Call(name, arity))
            }
            "call_parallel" => Ok(Op::CallParallel(parse_parallel_args(&parts[1..])?)),
            "call_race" => Ok(Op::CallRace(parse_parallel_args(&parts[1..])?)),
            "return" => Ok(Op::Return),
            "yield" => Ok(Op::Yield),
            "pop" => Ok(Op::Pop),
            "dup" => Ok(Op::Dup),
            "add" => Ok(Op::Add),
            "sub" => Ok(Op::Sub),
            "mul" => Ok(Op::Mul),
            "div" => Ok(Op::Div),
            "mod" => Ok(Op::Mod),
            "exp" => Ok(Op::Exp),
            "neg" => Ok(Op::Neg),
            "abs" => Ok(Op::Abs),
            "and" => Ok(Op::And),
            "or" => Ok(Op::Or),
            "not" => Ok(Op::Not),
            "xor" => Ok(Op::Xor),
            "eq" => Ok(Op::Eq),
            "ne" => Ok(Op::Ne),
            "lt" => Ok(Op::Lt),
            "le" => Ok(Op::Le),
            "gt" => Ok(Op::Gt),
            "ge" => Ok(Op::Ge),
            _ => {
                if parts[0].ends_with(":") {
                    Ok(Op::Label(parts[0][..parts[0].len()-1].to_string()))
                } else{
                    Err(Error::IRParse {
                        line: 0, // This will need to be backpatched.
                        msg: format!("Invalid op: '{}'", parts[0])
                    })
                }
            }
        }
    }
}

pub trait Callable: Send + Sync {
    fn call(&mut self) -> Result<bool, Error>;
    fn terminate(&mut self) -> Result<(), Error> {Ok(())}
    // fn arity(&self) -> usize;
}

pub trait CallableGenerator: Send + Sync {
    fn generate(&mut self, args: Vec<Value>) -> Result<Box<dyn Callable>, Error>;
    fn check_syntax(&self, args: Vec<Arg>) -> Result<(), Error>;
}

#[allow(unused_variables)]
pub trait Prop: Send + Sync {
    fn get(&self) -> Result<Value, Error>;
    fn set(&mut self, val: Value) -> Result<(), Error> {Ok(())}
    fn settable(&self) -> Result<bool, Error> {Ok(false)}
}

#[derive(Debug, PartialEq, Clone)]
pub enum Arg {
    Word(String),
    Value,
}

impl Arg {
    pub fn is_value(&self) -> bool {
        if let Arg::Value = self {
            true
        } else {
            false
        }
    }

    pub fn get_word(&self) -> Option<&str> {
        if let Arg::Word(s) = self {
            Some(&s)
        } else {
            None
        }
    }
}

impl Display for Arg {
    fn fmt(&self, f: &mut Formatter) -> FmtResult {
        match self {
            Arg::Word(s) => write!(f, "'{}'", s),
            Arg::Value => write!(f, "a value"),
        }
    }
}


#[derive(Clone)]
struct GroupData {
    name: String,
    // address: isize,
    params: Vec<Arg>,
}

#[derive(Clone)]
struct CompiledGroup {
    data: GroupData,
    code: Vec<Op>
}

impl CallableGenerator for CompiledGroup {
    // fn call(&mut self, _args: Vec<Value>) -> bool {
    //     panic!("This type only exists for the compiler. 'call' should never be called.")
    // }
    // fn terminate(&mut self) {
    //     panic!("This type only exists for the compiler. 'terminate' should never be called.")
    // }
    fn check_syntax(&self, args: Vec<Arg>) -> Result<(), Error> {
        if args.len() != self.data.params.len() {
            return Err(Error::Call(format!("Call to '{}' expected {} arguments but got {}", 
                                            self.data.name, 
                                            self.data.params.len(), 
                                            args.len())));
        }
        for (i, (param, args)) in self.data.params.iter().zip(args.iter()).enumerate() {
            if param != args {
                return Err(Error::Call(format!("Argument at position {} should be {}", i, param)));
            }
        } 

        Ok(())
    }
    fn generate(&mut self, _args: Vec<Value>) -> Result<Box<dyn Callable>, Error> {
        panic!("This type only exists for the compiler. 'arity' should never be called.")
    }
}

pub struct Program {
    pub code: Vec<Op>,
    pub callables: HashMap<String, Box<dyn CallableGenerator>>,
    pub props: HashMap<String, Box<dyn Prop>>,
}

impl Program {
    pub fn export(&self) -> String {
        self.code.iter().join("\n")
    }
}


pub struct Compiler {
    groups: HashMap<String, CompiledGroup>,
    instructions: Vec<Op>,
    callables: HashMap<String, Box<dyn CallableGenerator>>,
    properties: HashMap<String, Box<dyn Prop>>,
    allowed_props: HashSet<String>,

    variables: Vec<HashMap<String, usize>>,
    errors: Vec<Error>,
    in_progress: AtomicBool,
}

unsafe impl Send for Compiler {}
unsafe impl Sync for Compiler {}

impl<'a> Compiler {
    pub fn new() -> Compiler {
        Compiler {
            groups: HashMap::new(),
            instructions: Vec::new(),
            callables: HashMap::new(),
            properties: HashMap::new(),
            allowed_props: HashSet::new(),
            variables: vec![HashMap::new()],
            errors: Vec::new(),
            in_progress: AtomicBool::new(false),
        }
    }

    pub fn package_program(&mut self, code: Vec<Op>) -> Program {
        Program {
            code,
            callables: std::mem::take(&mut self.callables),
            props: std::mem::take(&mut self.properties),
        }
    }

    pub fn compile_nonconsuming(&mut self, ast: Vec<Stmt<'a>>) -> Result<Vec<Op>, Vec<Error>> {
        self.in_progress.store(true, Ordering::Release);

        let (uses, program): (Vec<Stmt<'a>>, _) = ast.into_iter().partition(|stmt| {
            if let Stmt::Use(_) = stmt {true} else {false}
        });

        for us in uses {
            us.accept_mut(self);
        }

        let program = self.isolate(move |this| {
            for stmt in program.iter() {
                stmt.accept_mut(this);
            }
        });

        let group_code = self.isolate(|this| {
            for (_name, group) in this.groups.iter_mut() {
                this.instructions.extend(group.code.drain(..));
            }
        });
        
        self.instructions.push(Op::Jump(group_code.len() as isize + 1));
        self.instructions.extend(group_code);
        self.instructions.extend(program);
        
        // for inst in program.iter() {
        //     inst.accept_mut(&mut self);
        // }


        
        let res = if self.errors.is_empty() {
            Ok(std::mem::take(&mut self.instructions))
        } else {
            Err(std::mem::take(&mut self.errors))
        };

        self.in_progress.store(false, Ordering::Release);
        res
    }

    pub fn compile(mut self, ast: Vec<Stmt<'a>>) -> Result<Program, Vec<Error>> {
        // No need to worry about updating `in_progress`, since this completely consumes the
        // compiler
        let code = self.compile_nonconsuming(ast)?;
        let res = self.package_program(code);

        Ok(res)
    }

    pub fn register_callable(&mut self, name: &str, callable: Box<dyn CallableGenerator>) -> Result<(), Error> {
        if self.in_progress.load(Ordering::Acquire) {
            return Err(Error::CompilerActive);
        }
        if self.callables.contains_key(name) {
            return Err(Error::DuplicateCallable(name.into()));
        }
        self.callables.insert(name.to_string(), callable);
        Ok(())
    }

    pub fn register_property(&mut self, name: &str, prop: Box<dyn Prop>) -> Result<(), Error> {
        if self.in_progress.load(Ordering::Acquire) {
            return Err(Error::CompilerActive);
        }
        if self.properties.contains_key(name) {
            return Err(Error::DuplicateProperty(name.into()));
        }
        self.properties.insert(name.to_string(), prop);
        Ok(())
    }

    fn declare_var(&mut self, name: &'a str) -> usize {
        let scope = unsafe {self.variables.last_mut().unwrap_unchecked()};
        if scope.contains_key(name) {
            return scope[name];
        }
        let idx = scope.len();
        scope.insert(name.to_string(), idx);
        idx
    }

    fn begin_scope(&mut self) {
        self.variables.push(HashMap::new());
    }

    fn end_scope(&mut self) {
        self.variables.pop();
    }

    fn get_var(&mut self, name: &'a str) -> Result<usize, Error> {
        let scope = unsafe {self.variables.last_mut().unwrap_unchecked()};
        scope.get(name).map(|i| *i).ok_or(Error::Compile{line: 0, msg: format!("Variable '{}' used before it was declared", name)})
    }

    // FIXME if useful, change output to Result<_> Halting parsing may be useful?
    fn isolate<F: FnMut(&mut Self)>(&mut self, mut f: F) -> Vec<Op> {
        let current_program = std::mem::take(&mut self.instructions);
        f(self);
        let sub_program = std::mem::replace(&mut self.instructions, current_program);
        sub_program
    }

    fn current_ip(&self) -> isize {
        self.instructions.len() as isize
    }
}

impl<'a> ExprVisitorMut<'a, ()> for Compiler {
    fn visit_variable_expr(&mut self, expr: &Variable<'a>) {
        let LexLiteral::Ident(name) = expr.name.literal.unwrap() else {
            self.errors.push(Error::Compile{line: 0, msg: "Variable was somehow called without an associated identifier".into()});
            return;
        };

        if self.properties.contains_key(name) {
            if !self.allowed_props.contains(name) {
                self.errors.push(Error::UndeclaredProperty(name.into()));
                return;
            }
            self.instructions.push(Op::Get(name.to_string()));
            return;
        }

        let idx = match self.get_var(name) {
            Ok(idx) => idx,
            Err(e) => {
                self.errors.push(e);
                return;
            }
        };
        self.instructions.push(Op::Load(idx));
    }
    
    fn visit_literal_expr(&mut self, expr: &Literal<'a>) {
        let value = match expr.value {
            LexLiteral::String(s) => Value::String(s.into()),
            LexLiteral::Number(n) => Value::Number(n),
            LexLiteral::Bool(b) => Value::Bool(b),
            LexLiteral::Nil => Value::Nil,
            LexLiteral::Ident(_) => panic!("You are actually using literal identifiers, stupid!"),
        };

        self.instructions.push(Op::Push(value))
    }
    
    fn visit_grouping_expr(&mut self, expr: &Grouping<'a>) {
        expr.expression.accept_mut(self);
        if expr.abs {
            self.instructions.push(Op::Abs);
        }
    }

    fn visit_unary_expr(&mut self, expr: &Unary<'a>) {
        expr.right.accept_mut(self);
        match expr.op.ty {
            TokenType::Not => self.instructions.push(Op::Not),
            TokenType::Minus => self.instructions.push(Op::Neg),
            _ => self.errors.push(Error::Compile{line: 0, msg: "Invalid unary operator".into()}),
        }
    }

    fn visit_binary_expr(&mut self, expr: &Binary<'a>) {
        expr.left.accept_mut(self);
        expr.right.accept_mut(self);

        match expr.op.ty {
            TokenType::BangEqual => self.instructions.push(Op::Ne),
            TokenType::EqualEqual => self.instructions.push(Op::Eq),
            TokenType::Greater => self.instructions.push(Op::Gt),
            TokenType::GreaterEqual => self.instructions.push(Op::Ge),
            TokenType::Less => self.instructions.push(Op::Lt),
            TokenType::LessEqual => self.instructions.push(Op::Le),
            TokenType::Minus => self.instructions.push(Op::Sub),
            TokenType::Plus => self.instructions.push(Op::Add),
            TokenType::Slash => self.instructions.push(Op::Div),
            TokenType::Star => self.instructions.push(Op::Mul),
            TokenType::Percent => self.instructions.push(Op::Mod),
            TokenType::Caret => self.instructions.push(Op::Exp),
            _ => self.errors.push(Error::Compile{line: 0, msg: "Invalid binary operator".into()}),
        }
    }

    // binary_expr!('a, or, xor, logical, [Or]);
    // binary_expr!('a, and, equality, logical, [And]);
    fn visit_logical_expr(&mut self, expr: &Logical<'a>) {
        expr.left.accept_mut(self);
        self.instructions.push(Op::Dup);

        match expr.op.ty {
            TokenType::And => {
                let sub_program = self.isolate(|this| {
                    expr.right.accept_mut(this);
                });
                self.instructions.push(Op::JumpUnless(sub_program.len() as isize + 1));
                self.instructions.extend(sub_program);
                self.instructions.push(Op::And);
            }
            TokenType::Or => {
                let sub_program = self.isolate(|this| {
                    expr.left.accept_mut(this);
                });
                self.instructions.push(Op::JumpIf(sub_program.len() as isize + 1));
                self.instructions.extend(sub_program);
                self.instructions.push(Op::Or);
            }
            TokenType::Xor => {
                expr.right.accept_mut(self);
                self.instructions.push(Op::Xor);
            }
            _ => {
                self.errors.push(Error::Compile{line: 0, msg: "Invalid logical operator".into()})
            }
        }
    }
}

impl<'a> StmtVisitorMut<'a, ()> for Compiler {
    fn visit_var_stmt(&mut self, stmt: &Var<'a>) {
        stmt.value.accept_mut(self);
        let LexLiteral::Ident(name) = stmt.name.literal.unwrap() else {
            self.errors.push(Error::Compile{line: 0, msg: "Invalid variable name".into()});
            return;
        };

        if let Some(prop) = self.properties.get(name) {
            match prop.settable() {
                Ok(false) => {
                    self.errors.push(Error::Compile{line: 0, msg: format!("External property '{}' not settable", name)});
                    return;
                }
                Err(e) => {
                    self.errors.push(e);
                    return;
                }
                _ => {}
            }
            self.instructions.push(Op::Set(name.into()));
        } else {
            let idx = self.declare_var(name);
            self.instructions.push(Op::Store(idx));
        }
    }

    fn visit_use_stmt(&mut self, stmt: &Use<'a>) {
        let LexLiteral::Ident(name) = stmt.name.literal.unwrap() else {
            self.errors.push(Error::Compile{line: 0, msg: "'use' statements must only use identifiers".into()});
            return;
        };

        if !self.properties.contains_key(name) {
            self.errors.push(Error::UnknownProperty(name.into()));
            return;
        }

        self.allowed_props.insert(name.into());
    }
    
    fn visit_exec_stmt(&mut self, stmt: &Exec<'a>) {
        let name = stmt.name.lexeme;
        if !self.groups.contains_key(name) && !self.callables.contains_key(name) {
            self.errors.push(Error::UnknownCallable(name.into()));
            return;
        } 

        let arg_kinds = stmt.args.iter().map(|a| {
            match a {
                AstArg::Word(w) => Arg::Word(w.lexeme.into()),
                AstArg::Value(_) => Arg::Value,
            }
        }).collect();

        if let Some(callable) = self.groups.get(name) {
            if let Err(e) = callable.check_syntax(arg_kinds) {
                self.errors.push(e);
                return;
            }
        } else {
            if let Err(e) = self.callables[name].check_syntax(arg_kinds) {
                self.errors.push(e);
                return;
            }
        }

        let mut arity = 0;
        for arg in stmt.args.iter() {
            if let AstArg::Value(v) = arg {
                arity += 1;
                v.accept_mut(self);
            }
        }

        self.instructions.push(Op::Call(name.into(), arity));
    }


    fn visit_parallel_stmt(&mut self, stmt: &Parallel<'a>) {
        let mut call_args = Vec::new();
        for call in stmt.calls.iter() {
            let name = call.name.lexeme;
            if !self.groups.contains_key(name) && !self.callables.contains_key(name) {
                self.errors.push(Error::UnknownCallable(name.into()));
                return;
            } 
        
            let arg_kinds = call.args.iter().map(|a| {
                match a {
                    AstArg::Word(w) => Arg::Word(w.lexeme.into()),
                    AstArg::Value(_) => Arg::Value,
                }
            }).collect();

            let arity = call.args.iter().filter(|a| if let AstArg::Value(_) = a {true} else {false}).count();
            let name = if let Some(callable) = self.groups.get(name) {
                if let Err(e) = callable.check_syntax(arg_kinds) {
                    self.errors.push(e);
                    return;
                }
                name.to_string()
            } else {
                if let Err(e) = self.callables[name].check_syntax(arg_kinds.clone()) {
                    self.errors.push(e);
                    return;
                }

                let anonymous_name = format!("#{}", name);
                
                if self.groups.contains_key(name) {
                    continue;
                }

                let external_group = CompiledGroup {
                    data: GroupData {
                        name: anonymous_name.clone(),
                        params: arg_kinds,
                    },
                    code: vec![
                        // There's no need to push call arguments on the stack since the groups args 
                        // are already in the right order for the call, and the immediate return
                        // will ensure the either the stack frame is discarded or the context is
                        // destroyed. Either way, the stack is well-formed.
                        Op::Label(anonymous_name.clone()),
                        Op::Call(name.to_string(), arity),
                        Op::Return,
                    ]
                };

                self.groups.insert(anonymous_name.clone(), external_group);

                anonymous_name
            };
            call_args.push((name, arity));
        }
        
        let prep = self.isolate(|this| {
            for call in stmt.calls.iter().rev() {
                for arg in call.args.iter().rev() {
                    match arg {
                        AstArg::Value(expr) => expr.accept_mut(this),
                        AstArg::Word(_) => continue,
                    }
                }
            }
        });
        // let offset = prep.len();
        // self.instructions.push(Op::StartPara(stmt.calls.len(), call_args.len()));
        self.instructions.extend(prep);
        self.instructions.push(if stmt.race {
            Op::CallRace(call_args)
        } else {
            Op::CallParallel(call_args)
        });
        // Just have the parallel calls have a +1 at the end. It's either this or be even more
        // opaque with the unnnecessary StartPara thing.
        // self.instructions.push(Op::Jump(-(offset as isize + 1)));
    }

    fn visit_group_stmt(&mut self, stmt: &Group<'a>) {
        self.begin_scope();
        
        let name = stmt.name.lexeme.to_string();
    
        if self.callables.contains_key(&name) {
            self.errors.push(Error::DuplicateCallable(name));
            return;
        }

        let params: Vec<_> = stmt.params.iter().map(|t| {
            match t.ty {
                TokenType::Word => Arg::Word(t.lexeme.to_string()),
                TokenType::Ident => {
                    let LexLiteral::Ident(name) = t.literal.unwrap() else {unreachable!()};
                    self.declare_var(name);
                    Arg::Value
                }
                _ => unreachable!(),
            }
        }).collect();
        

        let body = self.isolate(|this| {
            this.instructions.push(Op::Label(name.clone()));
            for stmt in stmt.statements.iter() {
                stmt.accept_mut(this);
            }
            for _ in 0..params.len() {
                this.instructions.push(Op::Pop);
            }
            this.instructions.push(Op::Return);
        });
    
        let group = CompiledGroup {
            data: GroupData {
                name: name.clone(),
                params,
                // address: self.current_ip(),
            },
            code: body,
        };
        
        // self.instructions.extend(body);
        self.groups.insert(name.clone(), group);

        self.end_scope();
    }


    fn visit_if_stmt(&mut self, stmt: &If<'a>) {
        stmt.condition.accept_mut(self);
        if stmt.invert {
            self.instructions.push(Op::Not);
        }
        let then_branch = self.isolate(|this| {
            for then in stmt.then_branch.iter() {
                then.accept_mut(this);
            }
        });

        let else_branch = self.isolate(|this| {
            for els in stmt.else_branch.iter() {
                els.accept_mut(this);
            }
        });
        let then_len = then_branch.len() as isize;
        let else_len = else_branch.len() as isize;

        self.instructions.push(Op::JumpUnless(then_len + 1));
        self.instructions.extend(then_branch);
        self.instructions.push(Op::Jump(else_len + 1));
    }

    fn visit_while_stmt(&mut self, stmt: &While<'a>) {
        let condition = self.isolate(|this| {
            stmt.condition.accept_mut(this);
            if stmt.invert {
                this.instructions.push(Op::Not);
            }
        });
        let body = self.isolate(|this| {
            for line in stmt.body.iter() {
                line.accept_mut(this)
            }
        });

        let len = (condition.len() + body.len()) as isize;
        self.instructions.extend(condition);
        self.instructions.push(Op::JumpUnless(body.len() as isize + 2));
        self.instructions.extend(body);
        self.instructions.push(Op::Jump(-(len + 1)));

        let sofar = self.instructions.len();
        // backpatch any breaks
        for (i, inst) in self.instructions.iter_mut().enumerate() {
            if let Op::Jump(a) = inst {
                if *a == 0 {
                    *a = (sofar - i) as isize;
                }
            }
        }
    }

    fn visit_return_stmt(&mut self, _stmt: &Return<'a>) {
        self.instructions.push(Op::Return);
    }
    
    fn visit_yield_stmt(&mut self, _stmt: &Yield<'a>) {
        self.instructions.push(Op::Yield);
    }

    fn visit_break_stmt(&mut self, _stmt: &Break<'a>) {
        self.instructions.push(Op::Jump(0)); // We'll use 0 to indicate that backpatching is
                                             // necessary, since jump 0 doesn't make any sense
    }
}
