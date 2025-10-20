use std::collections::{HashMap, HashSet};

use crate::compiler::{Program, Op, Value, Callable, Prop};
use crate::error::{Error};


struct StackFrame {
    return_addr: usize,
    stack_offset: usize,
}


#[derive(PartialEq)]
pub enum InterpreterState {
    Continue,
    Yield,
    Stop,
}

pub struct Interpreter {
    program: Vec<Op>,
    ip: usize,
    stack: Vec<Value>,
    call_stack: Vec<StackFrame>,
    props: HashMap<String, Box<dyn Prop>>,
    callables: HashMap<String, Box<dyn Callable>>,
    groups: HashMap<String, usize>,
    running: bool,
}

macro_rules! pop {
    ($self:expr) => {
        $self.stack.pop().ok_or(Error::StackUnderflow($self.ip - 1))
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
            (Value::Number(n) , Value::Number(m)) => $self.stack.push($res(m $op n)),
            (_, _) => {return Err(Error::Type("Both operands must be numbers".into()));},
        }
    }
}
macro_rules! logicop {
    ($self:expr, $op:tt) => {
        let a = pop!($self)?;
        let b = pop!($self)?;

        $self.stack.push(Value::Bool(a.truthy() $op b.truthy()));
    }
}

impl Interpreter {
    pub fn new(program: Vec<Op>) -> Interpreter {
        Interpreter {
            groups: Self::scan_groups(&program),
            program,
            ip: 0,
            stack: Vec::new(),
            call_stack: Vec::new(),
            props: HashMap::new(),
            callables: HashMap::new(),
            running: false,
        }
    }

    pub fn run(program: Program) -> Result<(), Error> {
        let mut interpreter = Interpreter {
            groups: Self::scan_groups(&program.code),
            program: program.code,
            ip: 0,
            stack: Vec::new(),
            call_stack: Vec::new(),
            props: program.props,
            callables: program.callables,
            running: false,
        };

        interpreter.interpret()
    }

    pub fn register_callable(&mut self, name: &str, callable: Box<dyn Callable>) -> Result<(), Error> {
        if self.running {
            return Err(Error::InterpreterActive);
        }
        if self.callables.contains_key(name) || self.groups.contains_key(name) {
            return Err(Error::DuplicateCallable(name.into()));
        }
        self.callables.insert(name.to_string(), callable);
        Ok(())
    }
    
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
        self.ip = 0;
        self.stack.clear();
        self.call_stack.clear();
    }

    pub fn step(&mut self) -> Result<InterpreterState, Error> {
        if !self.running {
            // FIXME This only returns the first error, which isn't ideal.
            if let Err(es) = self.verify_externals() {
                return Err(es[0].clone());
            }
        }
        self.running = true;
        let Some(op) = self.program.get(self.ip) else {
            // If at any point we go over the end, this indicates termination.
            self.running = false;
            return Ok(InterpreterState::Stop);
        };
        self.ip += 1;

        use Op::*;
        match op {
            Load(a) => {
                let offset = self.stack_offset();
                let value = self.stack.get(offset + a).ok_or(Error::IndexOutOfBounds(self.ip - 1))?;
                self.stack.push(value.clone());
            }
            Store(a) => {
                let offset = self.stack_offset();
                let value = pop!(self)?;
                let slot = self.stack.get_mut(offset + a).ok_or(Error::IndexOutOfBounds(self.ip - 1))?;
                *slot = value;
            }
            Get(name) => {
                // we assume the property exists at this point
                let value = self.props[name].get();
                self.stack.push(value);
            }
            Set(name) => {
                // we assume the property exists and is settable at this point
                let value = pop!(self)?;
                self.props.get_mut(name).unwrap().set(value);
            }
            Push(v) => self.stack.push(v.clone()),
            Pop => {self.stack.pop();},
            Dup => self.stack.push(self.stack.last().ok_or(Error::StackUnderflow(self.ip - 1))?.clone()),
            Add => {
                let a = pop!(self)?;
                let b = pop!(self)?;
                
                let value = match (a,b) {
                    (Value::Number(n), Value::Number(m)) => Value::Number(m + n),
                    (Value::String(s), Value::String(t)) => Value::String(t + &s),
                    (Value::Number(_), _) => {return Err(Error::Type("Right operand must be a number".into()));},
                    (Value::String(_), _) => {return Err(Error::Type("Right operand must be a string".into()));},
                    (_, _) => {return Err(Error::Type("Operands must be a number or a string".into()));}
                };
                self.stack.push(value);
            }
            Sub => {binop!(self, -);}
            Mul => {binop!(self, *);}
            Div => {binop!(self, /);}
            Mod => {binop!(self, %);}
            Exp => {
                let a = pop!(self)?;
                let b = pop!(self)?;

                match (a, b) {
                    (Value::Number(n) ,Value::Number(m)) => self.stack.push(Value::Number(m.powf(n))),
                    (_, _) => {return Err(Error::Type("Both operands must be numbers".into()));},
                }
            }
            Neg => {
                match self.stack.last_mut() {
                    Some(Value::Number(n)) => {*n = -*n;},
                    None => {return Err(Error::StackUnderflow(self.ip - 1))}
                    _ => {return Err(Error::Type("Only numbers can be negated".into()));},
                }
            }
            Abs => {
                match self.stack.last_mut() {
                    Some(Value::Number(n)) => {*n = n.abs();},
                    None => {return Err(Error::StackUnderflow(self.ip - 1))}
                    _ => {return Err(Error::Type("Absolute value only works with numbers".into()))}
                }
            }
            And => {logicop!(self, &&);}
            Or => {logicop!(self, ||);}
            Xor => {
                let a = pop!(self)?;
                let b = pop!(self)?;

                let a = a.truthy();
                let b = b.truthy();

                self.stack.push(Value::Bool(a && !b || b && !a));
            }
            Eq => {
                let a = pop!(self)?;
                let b = pop!(self)?;
                self.stack.push(Value::Bool(a == b));
            }
            Ne => {
                let a = pop!(self)?;
                let b = pop!(self)?;
                self.stack.push(Value::Bool(a != b));
            }
            Lt => {binop!(self, Value::Bool, <);}
            Le => {binop!(self, Value::Bool, <=);}
            Gt => {binop!(self, Value::Bool, >);}
            Ge => {binop!(self, Value::Bool, >=);}

            Jump(a) => {self.ip = *a;}
            JumpUnless(a) => {
                let cond = pop!(self)?;
                if !cond.truthy() {self.ip = *a;}
            }
            JumpIf(a) => {
                let cond = pop!(self)?;
                if cond.truthy() {self.ip = *a;}
            }

            Label(name) => {
                // No-op. Artefact of group identification.
            }
            Call(name) => {
                // name almost definitely (if not absolutely) exists at this point
                if let Some(callable) = self.callables.get_mut(name) {
                    if !callable.call() {
                        return Ok(InterpreterState::Yield);
                    }
                } else {
                    let Some(addr) = self.groups.get(name) else {
                        return Err(Error::UnregisteredCallable(self.ip - 1, name.into()));
                    };
                    self.call_stack.push(StackFrame {
                        return_addr: self.ip,
                        stack_offset: self.stack.len(),
                    });
                    self.ip = *addr;
                }
            }
            CallParallel(name) => {
                // TODO Because of this "parallelism", active built-in groups will need to be
                // tracked individually, including their own stacks and state tracking. i.e. they 
                // might need to be "instantiated" as their own callables.
            }

            _ => todo!()
        }
        Ok(InterpreterState::Continue)
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
                Op::Call(name) => {
                    if !(self.callables.contains_key(name) || self.groups.contains_key(name)) && !seen.contains(name) {
                        seen.insert(name.clone());
                        errors.push(Error::UnregisteredCallable(i, name.into()));
                    }
                }
                Op::CallParallel(name) | Op::CallRace(name) => {
                    if !self.groups.contains_key(name) && !seen.contains(name) {
                        seen.insert(name.clone());
                        errors.push(Error::InvalidCall(i));
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

    fn stack_offset(&self) -> usize {
        self.call_stack.last().map(|frame| frame.stack_offset).unwrap_or(0)
    }

}
