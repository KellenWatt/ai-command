#![allow(dead_code)]
use std::collections::{HashMap, HashSet, VecDeque};

use crate::compiler::{Program, Op, Value, Callable, CallableGenerator, Prop};
use crate::ast::{GroupKind};
use crate::error::{Error};


#[derive(Debug)]
struct StackFrame {
    return_addr: usize,
    stack_offset: usize,
    parallel_completion: Vec<bool>,
}

#[derive(Debug)]
struct ExecutionContext {
    call_stack: Vec<StackFrame>,
    stack: VecDeque<Value>,
    ip: usize,
    dependencies: Vec<ExecutionContext>,
    active: bool,
    dependency_type: GroupKind,
    parent: Option<*mut ExecutionContext>,
    native_proxy: bool,
    current_callable: Option<u32>,
}

impl ExecutionContext {
    fn new(start: usize) -> ExecutionContext {
        ExecutionContext {
            call_stack: Vec::new(),
            stack: VecDeque::new(),
            ip: start,
            dependencies: Vec::new(),
            active: true,
            dependency_type: GroupKind::Sequence,
            parent: None,
            native_proxy: false,
            current_callable: None,
        }
    }

    fn add_dependency(&mut self, mut ctx: ExecutionContext) {
        ctx.parent = Some(self as *mut ExecutionContext);
        self.dependencies.push(ctx);
    }
    
    fn set_native_proxy(&mut self) {
        self.native_proxy = true;
    }

    fn make_sequential(&mut self) {
        self.dependency_type = GroupKind::Sequence;
    }

    fn make_parallel(&mut self) {
        self.dependency_type = GroupKind::Parallel;
    }
    
    fn make_race(&mut self) {
        self.dependency_type = GroupKind::Race;
    }

    fn finalize(&mut self) {
        self.active = false;
    }

    fn stack_offset(&self) -> usize {
        self.call_stack.last().map(|frame| frame.stack_offset).unwrap_or(0)
    }
}


#[derive(PartialEq, Debug)]
enum ExecutionState {
    Continue,
    Yield,
    ThreadsAdded,
    CallEnd,
    Stop,
}

#[derive(PartialEq, Debug)]
pub enum InterpreterState {
    Yield,
    Stop,
}

pub struct Interpreter {
    program: Vec<Op>,
    root_context: ExecutionContext,
    // ip: usize,
    // stack: Vec<Value>,
    // call_stack: Vec<StackFrame>,
    props: HashMap<String, Box<dyn Prop>>,
    callables: HashMap<String, Box<dyn CallableGenerator>>,
    active_callables: HashMap<u32, Box<dyn Callable>>,
    callable_index: u32,
    groups: HashMap<String, usize>,
    ending: bool,
    running: bool,
}

macro_rules! pop {
    ($self:expr) => {
        $self.stack.pop_back().ok_or(Error::StackUnderflow($self.ip - 1))
    }
}

macro_rules! binop {
    ($self:expr, $op:tt) => {
        binop!($self, Value::Number, $op)
    };
    ($self:expr, $res: expr, $op:tt) => {
        let a = pop!($self)?;
        let b = pop!($self)?;

        match (a, b) {
            (Value::Number(n) , Value::Number(m)) => $self.stack.push_back($res(m $op n)),
            (_, _) => {return Err(Error::Type("Both operands must be numbers".into()));},
        }
    }
}
macro_rules! logicop {
    ($self:expr, $op:tt) => {
        let a = pop!($self)?;
        let b = pop!($self)?;

        $self.stack.push_back(Value::Bool(a.truthy() $op b.truthy()));
    }
}

impl Interpreter {
    #[allow(dead_code)]
    pub fn new(program: Vec<Op>) -> Interpreter {
        Interpreter {
            groups: Self::scan_groups(&program),
            program,
            root_context: ExecutionContext::new(0),
            props: HashMap::new(),
            callables: HashMap::new(),
            active_callables: HashMap::new(),
            callable_index: 0,
            ending: false,
            running: false,
        }
    }

    pub fn from_ir(program: &str) -> Result<Interpreter, Error> {
        let mut prog: Vec<Op> = Vec::new();
        for line in program.lines() {
            prog.push(line.parse()?);
        }
        Ok(Interpreter::new(prog))
    }

    pub fn from_program(program: Program) -> Interpreter {
        Interpreter {
            groups: Self::scan_groups(&program.code),
            program: program.code,
            root_context: ExecutionContext::new(0),
            props: program.props,
            callables: program.callables,
            active_callables: HashMap::new(),
            callable_index: 0,
            ending: false,
            running: false,
        }
    }

    #[allow(dead_code)]
    pub fn run(program: Program) -> Result<(), Error> {
        let mut interpreter = Interpreter {
            groups: Self::scan_groups(&program.code),
            program: program.code,
            root_context: ExecutionContext::new(0),
            props: program.props,
            callables: program.callables,
            active_callables: HashMap::new(),
            callable_index: 0,
            ending: false,
            running: false,
        };

        interpreter.interpret()
    }

    #[allow(dead_code)]
    pub fn register_callable(&mut self, name: &str, callable: Box<dyn CallableGenerator>) -> Result<(), Error> {
        if self.running {
            return Err(Error::InterpreterActive);
        }
        if self.callables.contains_key(name) || self.groups.contains_key(name) {
            return Err(Error::DuplicateCallable(name.into()));
        }
        self.callables.insert(name.to_string(), callable);
        Ok(())
    }
    
    #[allow(dead_code)]
    pub fn register_property(&mut self, name: &str, prop: Box<dyn Prop>) -> Result<(), Error> {
        if self.running {
            return Err(Error::InterpreterActive);
        }
        if self.props.contains_key(name) {
            return Err(Error::DuplicateProperty(name.into()));
        }
        self.props.insert(name.to_string(), prop);
        Ok(())
    }

    pub fn interpret(&mut self) -> Result<(), Error> {
        while self.step()? != InterpreterState::Stop {}
        Ok(())
    }

    pub fn reset(&mut self) {
        self.running = false;
        self.ending = false;
        self.active_callables.clear();
        self.callable_index = 0;
        self.root_context = ExecutionContext::new(0);
    }

    fn run_end(&mut self) -> Result<(), Error> {
        self.ending = true;
        self.root_context = ExecutionContext::new(0);
        if let Some(addr) = self.groups.get("__end") {
            self.root_context.ip = *addr;
            while let InterpreterState::Yield = self.step()? {}
        }
        self.running = false;
        Ok(())
    }

    pub fn end(&mut self) -> Result<(), Error> {
        unsafe {
            let mut stack: VecDeque<_> = vec![&mut self.root_context as *mut ExecutionContext].into();
            while !stack.is_empty() {
                let ctx = stack.pop_back().unwrap_unchecked();

                if (*ctx).dependencies.is_empty() {
                    if let Some(id) = (*ctx).current_callable {
                        if (*ctx).active {
                            self.active_callables.get_mut(&id).unwrap().terminate();
                        }
                        self.active_callables.remove(&id);
                    }
                }

                for dep in (*ctx).dependencies.iter_mut() {
                    stack.push_back(dep as *mut ExecutionContext);
                }
            }
        }
        self.run_end()?;

        Ok(())
    }

    pub fn step(&mut self) -> Result<InterpreterState, Error> {
        if !self.running {
            // FIXME This only returns the first error, which isn't ideal.
            if let Err(es) = self.verify_externals() {
                return Err(es[0].clone());
            }
        }
        self.running = true;

        unsafe {
            let mut queue: VecDeque<_> = vec![&mut self.root_context as *mut ExecutionContext].into();
            while !queue.is_empty() {
                // println!("{:?}", queue);
                let ctx = queue.pop_front().unwrap_unchecked();

                if !(*ctx).active {
                    continue;
                }

                    
                if (*ctx).dependency_type == GroupKind::Race && (*ctx).dependencies.iter().any(|c| !c.active){
                    // for dep in (*ctx).dependencies.iter_mut() {
                    //     if dep.active && dep.native_proxy {
                    //         let Some(Op::Call(name, _)) = self.program.get(dep.ip) else {unreachable!()};
                    //         let Some(callable) = self.callables.get_mut(name) else {unreachable!()};
                    //         callable.terminate();
                    //     }
                    // }
                    for dep in (*ctx).dependencies.iter_mut() {
                        if dep.current_callable.is_some() {
                            let Some(id) = dep.current_callable.take() else {unreachable!()};
                            if dep.active {
                                self.active_callables.get_mut(&id).unwrap().terminate();
                            }
                            self.active_callables.remove(&id);
                        }
                    }
                    (*ctx).dependencies.clear();
                } else {
                    (*ctx).dependencies.retain(|c| c.active);
                }

                
                if (*ctx).dependencies.len() == 0 {
                    (*ctx).make_sequential();
                    loop {
                        match self.step_with(ctx.as_mut().unwrap_unchecked())? {
                            ExecutionState::Continue => {continue;}
                            ExecutionState::Yield => {break;}
                            ExecutionState::ThreadsAdded => { 
                                for dep in (*ctx).dependencies.iter_mut() {
                                    queue.push_back(dep as *mut ExecutionContext);
                                }
                                break;
                            }
                            ExecutionState::CallEnd => {
                                if (*ctx).native_proxy {
                                    (*ctx).finalize();
                                    // root will never be a native proxy
                                    let Some(parent) = (*ctx).parent else {unreachable!()};
                                    if (*parent).dependency_type == GroupKind::Race {
                                        for dep in (*parent).dependencies.iter_mut() {
                                            if dep.current_callable.is_some() {
                                                let Some(id) = dep.current_callable.take() else {unreachable!()};
                                                if dep.active {
                                                    self.active_callables.get_mut(&id).unwrap().terminate();
                                                }
                                                self.active_callables.remove(&id);
                                            }
                                            // if dep.active && dep.native_proxy {
                                            //     let Some(Op::Call(name, _)) = self.program.get(dep.ip) else {unreachable!()};
                                            //     let Some(callable) = self.callables.get_mut(name) else {unreachable!()};
                                            //     callable.terminate();
                                            //     dep.finalize();
                                            // }
                                        }
                                    }
                                }
                                break;
                            }
                            ExecutionState::Stop => {
                                (*ctx).finalize(); 
                                if let Some(parent) = (*ctx).parent {
                                    if (*parent).dependency_type == GroupKind::Race {
                                        for dep in (*parent).dependencies.iter_mut() {
                                            if dep.current_callable.is_some() {
                                                let Some(id) = dep.current_callable.take() else {unreachable!()};
                                                if dep.active {
                                                    self.active_callables.get_mut(&id).unwrap().terminate();
                                                }
                                                self.active_callables.remove(&id);
                                            }
                                            // if dep.active && dep.native_proxy {
                                            //     let Some(Op::Call(name, _)) = self.program.get(dep.ip) else {unreachable!()};
                                            //     let Some(callable) = self.callables.get_mut(name) else {unreachable!()};
                                            //     callable.terminate();
                                            //     dep.finalize();
                                            // }
                                        }
                                        queue.push_back(parent);
                                    }
                                }
                                break;
                            }
                        }
                    }
                } else {
                    for dep in (*ctx).dependencies.iter_mut() {
                        queue.push_back(dep as *mut ExecutionContext);
                    }
                }
            }
        }

        if !self.root_context.active {
            if !self.ending {
                self.run_end()?;
            }
            Ok(InterpreterState::Stop)
        } else {
            Ok(InterpreterState::Yield)
        }
    }

    fn step_with(&mut self, ctx: &mut ExecutionContext) -> Result<ExecutionState, Error> {
        // println!("{:?}", ctx);
        // println!("{} {:?}", ctx.ip, ctx.stack);
        let Some(op) = self.program.get(ctx.ip) else {
            // println!("IP out of bounds");
            return Ok(ExecutionState::Stop);
        };
        ctx.ip += 1;
       
        use Op::*;
        match op {
            Load(a) => {
                let offset = ctx.stack_offset();
                let value = ctx.stack.get(offset + a).ok_or(Error::IndexOutOfBounds(ctx.ip - 1))?;
                ctx.stack.push_back(value.clone());
            }
            Store(a) => {
                let offset = ctx.stack_offset();
                if ctx.stack.len() > offset + a + 1 {
                    let value = pop!(ctx)?;
                    let slot = ctx.stack.get_mut(offset + a).ok_or(Error::IndexOutOfBounds(ctx.ip - 1))?;
                    *slot = value;
                }
            }
            Get(name) => {
                // we assume the property exists at this point
                let value = self.props[name].get();
                ctx.stack.push_back(value);
            }
            Set(name) => {
                // we assume the property exists and is settable at this point
                let value = pop!(ctx)?;
                self.props.get_mut(name).unwrap().set(value);
            }
            Push(v) => ctx.stack.push_back(v.clone()),
            Pop => {ctx.stack.pop_back();},
            Dup => ctx.stack.push_back(ctx.stack.back().ok_or(Error::StackUnderflow(ctx.ip - 1))?.clone()),
            Add => {
                let a = pop!(ctx)?;
                let b = pop!(ctx)?;
               
                let value = match (a,b) {
                    (Value::Number(n), Value::Number(m)) => Value::Number(m + n),
                    (Value::String(s), Value::String(t)) => Value::String(t + &s),
                    (Value::Number(_), _) => {return Err(Error::Type("Right operand must be a number".into()));},
                    (Value::String(_), _) => {return Err(Error::Type("Right operand must be a string".into()));},
                    (_, _) => {return Err(Error::Type("Operands must be a number or a string".into()));}
                };
                ctx.stack.push_back(value);
            }
            Sub => {binop!(ctx, -);}
            Mul => {binop!(ctx, *);}
            Div => {binop!(ctx, /);}
            Mod => {binop!(ctx, %);}
            Exp => {
                let a = pop!(ctx)?;
                let b = pop!(ctx)?;

                match (a, b) {
                    (Value::Number(n) ,Value::Number(m)) => ctx.stack.push_back(Value::Number(m.powf(n))),
                    (_, _) => {return Err(Error::Type("Both operands must be numbers".into()));},
                }
            }
            Neg => {
                match ctx.stack.back_mut() {
                    Some(Value::Number(n)) => {*n = -*n;},
                    None => {return Err(Error::StackUnderflow(ctx.ip - 1))}
                    _ => {return Err(Error::Type("Only numbers can be negated".into()));},
                }
            }
            Abs => {
                match ctx.stack.back_mut() {
                    Some(Value::Number(n)) => {*n = n.abs();},
                    None => {return Err(Error::StackUnderflow(ctx.ip - 1))}
                    _ => {return Err(Error::Type("Absolute value only works with numbers".into()))}
                }
            }
            And => {logicop!(ctx, &&);}
            Or => {logicop!(ctx, ||);}
            Not => {
                match ctx.stack.back_mut() {
                    Some(v) => {*v = Value::Bool(!v.truthy());},
                    None => {return Err(Error::StackUnderflow(ctx.ip - 1))}
                }
            }
            Xor => {
                let a = pop!(ctx)?;
                let b = pop!(ctx)?;

                let a = a.truthy();
                let b = b.truthy();

                ctx.stack.push_back(Value::Bool(a && !b || b && !a));
            }
            Eq => {
                let a = pop!(ctx)?;
                let b = pop!(ctx)?;
                ctx.stack.push_back(Value::Bool(a == b));
            }
            Ne => {
                let a = pop!(ctx)?;
                let b = pop!(ctx)?;
                ctx.stack.push_back(Value::Bool(a != b));
            }
            Lt => {binop!(ctx, Value::Bool, <);}
            Le => {binop!(ctx, Value::Bool, <=);}
            Gt => {binop!(ctx, Value::Bool, >);}
            Ge => {binop!(ctx, Value::Bool, >=);}

            Jump(a) => {ctx.ip = ctx.ip.wrapping_add_signed(*a - 1);}
            JumpUnless(a) => {
                let cond = pop!(ctx)?;
                if !cond.truthy() {ctx.ip = ctx.ip.wrapping_add_signed(*a - 1);}
            }
            JumpIf(a) => {
                let cond = pop!(ctx)?;
                if cond.truthy() {ctx.ip = ctx.ip.wrapping_add_signed(*a - 1);}
            }

            Label(_name) => {
                // No-op. Artefact of group identification.
            }
            Call(name, arity) => {
                if let Some(gener) = self.callables.get_mut(name) {
                    if ctx.current_callable.is_none() {
                        let new_callable = gener.generate();
                        ctx.current_callable = Some(self.callable_index);
                        self.active_callables.insert(self.callable_index, new_callable);
                        self.callable_index += 1;
                    };
                    let callable = self.active_callables.get_mut(ctx.current_callable.as_ref().unwrap()).unwrap();

                    let mut args = Vec::new();
                    for _ in 0..*arity {
                        args.push(pop!(ctx)?);
                    }
                    args.reverse();

                    if !callable.call(args) {
                        ctx.ip -= 1;
                        return Ok(ExecutionState::Yield);
                    } else {
                        let Some(id) = ctx.current_callable.take() else {unreachable!()};
                        self.active_callables.remove(&id);
                        return Ok(ExecutionState::CallEnd);
                    }
                } else {
                    let Some(addr) = self.groups.get(name) else {
                        return Err(Error::UnregisteredCallable(ctx.ip - 1, name.into()));
                    };
                    ctx.call_stack.push(StackFrame {
                        return_addr: ctx.ip,
                        stack_offset: ctx.stack.len() - arity,
                        parallel_completion: Vec::new(),
                    });
                    ctx.ip = *addr;
                }
            }

            CallParallel(calls) => {
                ctx.make_parallel();
                for (name, arity) in calls.iter() {
                    if let Some(addr) = self.groups.get(name) {
                        let mut sub_ctx = ExecutionContext::new(*addr);
                        // println!("claiming {} arguments for {}", arity, name);
                        for _ in 0..*arity {
                            let val = pop!(ctx)?;
                            sub_ctx.stack.push_back(val);
                        }
                        if name.starts_with("#") {
                            sub_ctx.set_native_proxy();
                        }
                        ctx.add_dependency(sub_ctx);
                    } else {
                        return Err(Error::InvalidCall(ctx.ip - 1));
                    }
                }
                // ctx.ip += 1;
                // This is not the intended use of this state, but it seems to be necessary.
                return Ok(ExecutionState::ThreadsAdded);
            }
            CallRace(calls) => {
                ctx.make_race();
                // let container = ExecutionContext::new(ctx.ip+1);
                for (name, arity) in calls.iter() {
                    if let Some(addr) = self.groups.get(name) {
                        let mut sub_ctx = ExecutionContext::new(*addr);
                        // println!("claiming {} arguments for {}", arity, name);
                        for _ in 0..*arity {
                            let val = pop!(ctx)?;
                            sub_ctx.stack.push_back(val);
                        }
                        if name.starts_with("#") {
                            sub_ctx.set_native_proxy();
                        }
                        ctx.add_dependency(sub_ctx);
                    } else {
                        return Err(Error::InvalidCall(ctx.ip - 1));
                    }
                }
                // ctx.ip += 1;
                // This is not the intended use of this state, but it seems to be necessary.
                return Ok(ExecutionState::ThreadsAdded);

            }
            Return => {
                // This interpretation opens up the possibiilty of a naked return, but I don't know
                // that's a problem?
                match ctx.call_stack.pop() {
                    Some(frame) => {
                        ctx.ip = frame.return_addr;
                    }
                    None => {
                        return Ok(ExecutionState::Stop);
                    }
                }
            }

            // _ => todo!()
        }
        Ok(ExecutionState::Continue)
    }


    fn scan_groups(program: &[Op]) -> HashMap<String, usize> {
        let mut groups = HashMap::new();
        for (i, op) in program.iter().enumerate() {
            if let Op::Label(name) = op {
                groups.insert(name.clone(), i);
            }
        }
        groups
    }

    fn verify_externals(&self) -> Result<(), Vec<Error>> {
        let mut errors = Vec::new();
        let mut seen = HashSet::new();
        for (i, op) in self.program.iter().enumerate() {
            match op {
                Op::Get(name) => {
                    if !seen.contains(name) && !self.props.contains_key(name) {
                        seen.insert(name.clone());
                        errors.push(Error::UnregisteredProperty(i, name.into()));
                    }
                }
                Op::Set(name) => {
                    if let Some(prop) = self.props.get(name) {
                        if !prop.settable() && !seen.contains(name) {
                            seen.insert(name.clone());
                            errors.push(Error::UnsettableProperty(i, name.into()));
                        }
                    } else {
                        if !seen.contains(name) {
                            seen.insert(name.clone());
                            errors.push(Error::UnregisteredProperty(i, name.into()));
                        }
                    }
                }
                Op::Call(name, _) => {
                    if !(self.callables.contains_key(name) || self.groups.contains_key(name)) && !seen.contains(name) {
                        seen.insert(name.clone());
                        errors.push(Error::UnregisteredCallable(i, name.into()));
                    }
                }
                Op::CallParallel(calls) | Op::CallRace(calls) => {
                    for call in calls.iter() {
                        let name = &call.0;
                        if !(self.callables.contains_key(name) || self.groups.contains_key(name)) && !seen.contains(name) {
                            seen.insert(name.clone());
                            errors.push(Error::UnregisteredCallable(i, name.into()));
                        }
                    }
                }
                _ => {}
            }
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

}
