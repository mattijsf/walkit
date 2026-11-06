// SPDX-License-Identifier: GPL-3.0-or-later

//! The interpreter: runs a script's handlers on the runtime, which answers every method call.

use super::SkinRuntime;
use crate::maki::program::op;
use crate::maki::{Cell, Handle, Kind, Value};

const STEP_LIMIT: i64 = 2_000_000;
const MAX_STACK: usize = 65_536;
const MAX_CALL_DEPTH: usize = 1_024;
const MAX_STRING: usize = 1 << 20;

/// A slot on the stack: a script variable, a member variable, or a value of its own.
#[derive(Debug, Clone)]
enum Slot {
    Var(usize),
    Member((Handle, String)),
    Temp(Cell),
}

fn default_for(ret: u8) -> Value {
    match ret {
        b'I' | b'B' => Value::Int(0),
        b'F' | b'D' => Value::Num(0.0),
        b'S' => Value::str(""),
        _ => Value::Null,
    }
}

fn coerce_return(v: Value, ret: u8) -> Value {
    match ret {
        b'I' => match v {
            Value::Int(_) => v,
            _ => Value::Int(v.as_int()),
        },
        b'B' => Value::Int(v.truthy() as i32),
        b'F' | b'D' => match v {
            Value::Num(_) => v,
            _ => Value::Num(v.as_double()),
        },
        b'S' => match v {
            Value::Str(_) => v,
            _ => Value::Str(v.as_string()),
        },
        _ => v,
    }
}

fn equal(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Str(x), Value::Str(y)) => {
            x.eq_ignore_ascii_case(y) || x.to_lowercase() == y.to_lowercase()
        }
        _ if matches!(a, Value::Obj(_) | Value::Null)
            || matches!(b, Value::Obj(_) | Value::Null) =>
        {
            a.as_object() == b.as_object()
        }
        _ if matches!(a, Value::Str(_)) || matches!(b, Value::Str(_)) => {
            a.as_string().to_lowercase() == b.as_string().to_lowercase()
        }
        _ => a.as_double() == b.as_double(),
    }
}

fn ordinal(a: &Value, b: &Value) -> std::cmp::Ordering {
    match (a, b) {
        (Value::Str(x), Value::Str(y)) => x.to_lowercase().cmp(&y.to_lowercase()),
        _ => a
            .as_double()
            .partial_cmp(&b.as_double())
            .unwrap_or(std::cmp::Ordering::Equal),
    }
}

/// A value as Kotlin's `toString` of the Kotlin engine's values writes it, for a log line.
fn value_text(rt: &SkinRuntime, v: &Value) -> String {
    match v {
        Value::Obj(h) => format!("Obj({})", rt.kotlin_name(*h)),
        Value::Null => "null".to_string(),
        other => other.as_string().to_string(),
    }
}

impl SkinRuntime {
    /// Runs the handlers of script `s` for `event` on `target`: how many ran, and the last return.
    pub(crate) fn script_dispatch(
        &mut self,
        s: usize,
        target: Handle,
        event: &str,
        args: &[Value],
    ) -> (i32, Value) {
        let handlers = self.scripts[s].handlers(event, target);
        let mut ran = 0;
        let mut last = Value::Null;
        for (var, start, as_member) in handlers {
            if as_member {
                self.scripts[s].cells[var].assign(Value::Obj(target));
            }
            last = self.run_script(s, start, args, event);
            self.scripts[s].last_return = last.clone();
            ran += 1;
        }
        (ran, last)
    }

    /// Runs script `s` from `start` with `args`; whatever a handler throws is logged and ends
    /// that run, not the player.
    pub(crate) fn run_script(
        &mut self,
        s: usize,
        start: usize,
        args: &[Value],
        what: &str,
    ) -> Value {
        if self.scripts[s].disabled {
            return Value::Null;
        }
        match self.interpret(s, start, args) {
            Ok(v) => v,
            Err(why) => {
                let id = self.scripts[s].program.id.clone();
                self.host.log(&format!("{id}: {what} stopped: {why}"));
                if self.scripts[s].failed() {
                    let n = self.scripts[s].failures();
                    self.host
                        .log(&format!("{id}: switched off after {n} failures"));
                }
                Value::Null
            }
        }
    }

    fn slot_value(&mut self, s: usize, slot: &Slot) -> Value {
        match slot {
            Slot::Var(i) => self.scripts[s].cells[*i].value.clone(),
            Slot::Member(key) => self.scripts[s].member_cell(key).value.clone(),
            Slot::Temp(c) => c.value.clone(),
        }
    }

    fn slot_assign(&mut self, s: usize, slot: &mut Slot, v: Value) {
        match slot {
            Slot::Var(i) => self.scripts[s].cells[*i].assign(v),
            Slot::Member(key) => self.scripts[s].member_cell(key).assign(v),
            Slot::Temp(c) => c.assign(v),
        }
    }

    /// Runs a handler; returns what it left for its caller (the value pushed before its last RETURN).
    fn interpret(&mut self, s: usize, start: usize, args: &[Value]) -> Result<Value, String> {
        let program = self.scripts[s].program.clone();
        let code = &program.code;
        let mut stack: Vec<Slot> = Vec::new();
        let mut call_stack: Vec<usize> = Vec::new();
        let push = |stack: &mut Vec<Slot>, slot: Slot| -> Result<(), String> {
            if stack.len() >= MAX_STACK {
                return Err("stack overflow".into());
            }
            stack.push(slot);
            Ok(())
        };
        let push_value = |stack: &mut Vec<Slot>, v: Value| -> Result<(), String> {
            let kind = Kind::of(&v);
            push(stack, Slot::Temp(Cell::new(kind, v)))
        };
        // the handler's prologue pops its parameters, first parameter first
        for a in args.iter().rev() {
            push_value(&mut stack, a.clone())?;
        }
        let mut ip = start;
        let mut steps: i64 = 0;
        while ip < code.len() {
            let o = code[ip];
            steps += 1;
            if steps > STEP_LIMIT {
                return Err(format!("step limit at {}", o.offset));
            }
            self.scripts[s].budget -= 1;
            if self.scripts[s].budget < 0 {
                return Err(format!("frame budget used up at {}", o.offset));
            }
            let underflow = || format!("stack underflow at {} (opcode 0x{:x})", o.offset, o.opcode);
            macro_rules! pop {
                () => {
                    stack.pop().ok_or_else(underflow)?
                };
            }
            macro_rules! pop_value {
                () => {{
                    let slot = pop!();
                    self.slot_value(s, &slot)
                }};
            }
            match o.opcode {
                op::PUSH => {
                    let i = o.arg as usize;
                    if i >= self.scripts[s].cells.len() {
                        return Err(format!("variable {i} at {}", o.offset));
                    }
                    push(&mut stack, Slot::Var(i))?;
                }
                op::POP => {
                    pop!();
                }
                op::POP_TO => {
                    let v = pop_value!();
                    let i = o.arg as usize;
                    if i >= self.scripts[s].cells.len() {
                        return Err(format!("variable {i} at {}", o.offset));
                    }
                    self.scripts[s].cells[i].assign(v);
                }
                op::EQ | op::NE | op::GT | op::GE | op::LT | op::LE => {
                    let b = pop_value!();
                    let a = pop_value!();
                    let r = match o.opcode {
                        op::EQ => equal(&a, &b),
                        op::NE => !equal(&a, &b),
                        op::GT => ordinal(&a, &b).is_gt(),
                        op::GE => ordinal(&a, &b).is_ge(),
                        op::LT => ordinal(&a, &b).is_lt(),
                        _ => ordinal(&a, &b).is_le(),
                    };
                    push_value(&mut stack, Value::Int(r as i32))?;
                }
                op::JUMP_IF_NOT => {
                    if !pop_value!().truthy() {
                        ip = o.arg as usize;
                        continue;
                    }
                }
                op::JUMP_IF => {
                    if pop_value!().truthy() {
                        ip = o.arg as usize;
                        continue;
                    }
                }
                op::JUMP => {
                    ip = o.arg as usize;
                    continue;
                }
                op::CALL | op::CALL2 => {
                    self.call(s, &o, &mut stack)?;
                }
                op::CALL_GLOBAL => {
                    if call_stack.len() >= MAX_CALL_DEPTH {
                        return Err(format!("calls nested too deep at {}", o.offset));
                    }
                    call_stack.push(ip);
                    ip = o.arg as usize;
                    continue;
                }
                op::RETURN => match call_stack.pop() {
                    Some(back) => ip = back,
                    None => {
                        return Ok(match stack.last() {
                            Some(slot) => {
                                let slot = slot.clone();
                                self.slot_value(s, &slot)
                            }
                            None => Value::Null,
                        })
                    }
                },
                op::COMPLETE => {}
                op::MEMBER => {
                    let name = pop_value!().as_string();
                    let owner = pop_value!().as_object();
                    let kind = Kind::of_type(o.arg).unwrap_or(Kind::Object);
                    if let Some(owner) = owner {
                        let key = self.scripts[s].member(owner, &name, kind)?;
                        push(&mut stack, Slot::Member(key))?;
                    } else {
                        let id = program.id.clone();
                        self.host.log(&format!("{id}: member {name} on null"));
                        let mut cell = Cell::new(kind, Value::Null);
                        cell.assign(Value::Null);
                        push(&mut stack, Slot::Temp(cell))?;
                    }
                }
                op::MOV => {
                    let src = pop!();
                    let mut dst = pop!();
                    let v = self.slot_value(s, &src);
                    self.slot_assign(s, &mut dst, v);
                    push(&mut stack, src)?;
                }
                op::POST_INC | op::POST_DEC | op::PRE_INC | op::PRE_DEC => {
                    let delta = if matches!(o.opcode, op::POST_INC | op::PRE_INC) {
                        1
                    } else {
                        -1
                    };
                    let mut cell = pop!();
                    let old = self.slot_value(s, &cell);
                    let new = match old {
                        Value::Int(v) => Value::Int(v.wrapping_add(delta)),
                        _ => Value::Num(old.as_double() + delta as f64),
                    };
                    self.slot_assign(s, &mut cell, new);
                    if matches!(o.opcode, op::POST_INC | op::POST_DEC) {
                        push_value(&mut stack, old)?;
                    } else {
                        push(&mut stack, cell)?;
                    }
                }
                op::ADD | op::SUB | op::MUL | op::DIV => {
                    let b = pop_value!();
                    let a = pop_value!();
                    if matches!(a, Value::Str(_)) || matches!(b, Value::Str(_)) {
                        if o.opcode != op::ADD {
                            return Err(format!("string arithmetic at {}", o.offset));
                        }
                        let joined = format!("{}{}", a.as_string(), b.as_string());
                        if joined.len() > MAX_STRING {
                            return Err(format!("string longer than {MAX_STRING} at {}", o.offset));
                        }
                        push_value(&mut stack, Value::string(joined))?;
                        continue_ip(&mut ip);
                        continue;
                    }
                    let (x, y) = (a.as_double(), b.as_double());
                    let r = match o.opcode {
                        op::ADD => x + y,
                        op::SUB => x - y,
                        op::MUL => x * y,
                        _ => {
                            if y == 0.0 {
                                0.0
                            } else {
                                x / y
                            }
                        }
                    };
                    // Int op Int stays Int, except division, which the scripts use for ratios
                    if !matches!(a, Value::Num(_))
                        && !matches!(b, Value::Num(_))
                        && o.opcode != op::DIV
                    {
                        push_value(&mut stack, Value::Int(crate::maki::value::double_to_int(r)))?;
                    } else {
                        push_value(&mut stack, Value::Num(r))?;
                    }
                }
                op::MOD | op::AND | op::OR | op::XOR | op::SHL | op::SHL2 | op::SHR | op::SHR2 => {
                    let b = pop_value!().as_int();
                    let a = pop_value!().as_int();
                    let r = match o.opcode {
                        op::MOD => {
                            if b == 0 {
                                0
                            } else {
                                a.wrapping_rem(b)
                            }
                        }
                        op::AND => a & b,
                        op::OR => a | b,
                        op::XOR => a ^ b,
                        op::SHL | op::SHL2 => a.wrapping_shl(b as u32),
                        _ => a.wrapping_shr(b as u32),
                    };
                    push_value(&mut stack, Value::Int(r))?;
                }
                op::NOT => {
                    let v = pop_value!();
                    push_value(&mut stack, Value::Int(i32::from(!v.truthy())))?;
                }
                op::BIT_NOT => {
                    let v = pop_value!().as_int();
                    push_value(&mut stack, Value::Int(!v))?;
                }
                op::NEG => {
                    let v = pop_value!();
                    let r = match v {
                        Value::Int(i) => Value::Int(i.wrapping_neg()),
                        _ => Value::Num(-v.as_double()),
                    };
                    push_value(&mut stack, r)?;
                }
                op::LOG_AND | op::LOG_OR => {
                    let b = pop_value!();
                    let a = pop_value!();
                    let r = if o.opcode == op::LOG_AND {
                        a.truthy() && b.truthy()
                    } else {
                        a.truthy() || b.truthy()
                    };
                    push_value(&mut stack, Value::Int(r as i32))?;
                }
                op::NEW => {
                    let Some(guid) = program.classes.get(o.arg as usize) else {
                        return Err(format!("class {} at {}", o.arg, o.offset));
                    };
                    let made = self.create(guid);
                    if made.is_none() {
                        let id = program.id.clone();
                        let name = program.class_name(o.arg as usize);
                        self.host.log(&format!("{id}: cannot make {name}"));
                    }
                    push(
                        &mut stack,
                        Slot::Temp(Cell::new(Kind::Object, Value::of(made))),
                    )?;
                }
                op::DELETE => {
                    // `delete x` is an expression to the compiler, which drops its value with the POP
                    // that always follows: take the object, leave a value in its place
                    if let Some(h) = pop_value!().as_object() {
                        self.delete_object(h);
                        self.scripts[s].forget(h);
                    }
                    push(&mut stack, Slot::Temp(Cell::new(Kind::Object, Value::Null)))?;
                }
                other => return Err(format!("opcode 0x{other:x} at {}", o.offset)),
            }
            ip += 1;
        }
        Ok(Value::Null)
    }

    fn call(
        &mut self,
        s: usize,
        o: &crate::maki::program::Op,
        stack: &mut Vec<Slot>,
    ) -> Result<(), String> {
        let program = self.scripts[s].program.clone();
        let Some(method) = program.methods.get(o.arg as usize) else {
            return Err(format!("method {} at {}", o.arg, o.offset));
        };
        let api = self.scripts[s].api(o.arg as usize);
        let argc = if o.argc >= 0 {
            o.argc as usize
        } else {
            match api {
                Some(a) => a.params as usize,
                None => {
                    return Err(format!(
                        "unknown method {}.{}",
                        program.class_name(method.class_index),
                        method.name
                    ))
                }
            }
        };
        // the compiler pushes the last argument first, so the first argument is on top
        let mut args = Vec::with_capacity(argc);
        for _ in 0..argc {
            let slot = stack.pop().ok_or_else(|| {
                format!("stack underflow at {} (opcode 0x{:x})", o.offset, o.opcode)
            })?;
            args.push(self.slot_value(s, &slot));
        }
        let receiver_slot = stack
            .pop()
            .ok_or_else(|| format!("stack underflow at {} (opcode 0x{:x})", o.offset, o.opcode))?;
        let receiver = self.slot_value(s, &receiver_slot).as_object();
        let ret = api.map(|a| a.ret).unwrap_or(b'A');
        let push_value = |stack: &mut Vec<Slot>, v: Value| -> Result<(), String> {
            if stack.len() >= MAX_STACK {
                return Err("stack overflow".into());
            }
            let kind = Kind::of(&v);
            stack.push(Slot::Temp(Cell::new(kind, v)));
            Ok(())
        };
        let Some(receiver) = receiver else {
            self.host.log(&format!(
                "{}: {}.{} on null",
                program.id,
                program.class_name(method.class_index),
                method.name
            ));
            return push_value(stack, default_for(ret));
        };
        let mut result = self.maki_call(receiver, &method.name, &args);
        if let Some(e) = self.script_error.take() {
            return Err(e);
        }
        // a stand-in object hands out stand-ins, so a script keeps going through objects this runtime lacks
        if result == Some(Value::Null) && ret == b'O' && self.is_stub(receiver) {
            result = Some(Value::Obj(self.another_stub()));
        }
        if result.is_none() && method.name.starts_with("on") {
            // calling an event fires it, so other handlers (and this script's own) run
            result = Some(self.dispatch_for_result(receiver, &method.name, &args));
        }
        let Some(result) = result else {
            let joined = args
                .iter()
                .map(|a| value_text(self, a))
                .collect::<Vec<_>>()
                .join(", ");
            self.host.log(&format!(
                "{}: {}.{}({joined}) not implemented",
                program.id,
                self.kotlin_name(receiver),
                method.name
            ));
            return push_value(stack, default_for(ret));
        };
        if let Value::Str(st) = &result {
            if st.len() > MAX_STRING {
                return Err(format!(
                    "{} gave a string longer than {MAX_STRING}",
                    method.name
                ));
            }
        }
        push_value(stack, coerce_return(result, ret))
    }
}

fn continue_ip(ip: &mut usize) {
    *ip += 1;
}
