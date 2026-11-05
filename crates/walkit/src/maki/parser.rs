// SPDX-License-Identifier: GPL-3.0-or-later

//! Reads a `.maki` file: a fixed sequence of sections without headers, all little endian.
//!
//! The layout follows the community description of the bytecode (Webamp's maki-bytecode.md,
//! MIT): header, class GUIDs, method imports, variables, string constants, event bindings,
//! code, then optional debug data this reader ignores.

use std::collections::HashMap;

use crate::maki::program::{op, Binding, MethodRef, Op, Program, VarDecl};
use crate::maki::{Kind, Value};

const MAX_TABLE: i32 = 1 << 16;

struct Buf<'a> {
    b: &'a [u8],
    pos: usize,
    id: &'a str,
}

impl<'a> Buf<'a> {
    fn damaged(&self) -> String {
        format!("{}: damaged (read past the end)", self.id)
    }

    fn need(&self, n: usize) -> Result<(), String> {
        if self.pos + n > self.b.len() {
            Err(self.damaged())
        } else {
            Ok(())
        }
    }

    fn u8(&mut self) -> Result<u8, String> {
        self.need(1)?;
        self.pos += 1;
        Ok(self.b[self.pos - 1])
    }

    fn u16(&mut self) -> Result<u16, String> {
        self.need(2)?;
        let v = u16::from_le_bytes([self.b[self.pos], self.b[self.pos + 1]]);
        self.pos += 2;
        Ok(v)
    }

    fn i32(&mut self) -> Result<i32, String> {
        let v = self.peek_i32(self.pos)?;
        self.pos += 4;
        Ok(v)
    }

    fn peek_i32(&self, at: usize) -> Result<i32, String> {
        if at + 4 > self.b.len() {
            return Err(self.damaged());
        }
        Ok(i32::from_le_bytes([
            self.b[at],
            self.b[at + 1],
            self.b[at + 2],
            self.b[at + 3],
        ]))
    }

    fn bytes(&mut self, n: usize) -> Result<&'a [u8], String> {
        self.need(n)?;
        self.pos += n;
        Ok(&self.b[self.pos - n..self.pos])
    }

    fn remaining(&self) -> usize {
        self.b.len() - self.pos
    }
}

fn latin1(b: &[u8]) -> String {
    b.iter().map(|&c| c as char).collect()
}

/// The program in `bytes`, or why the file cannot be read.
pub fn parse(bytes: &[u8], id: &str) -> Result<Program, String> {
    let mut buf = Buf {
        b: bytes,
        pos: 0,
        id,
    };
    let magic = latin1(buf.bytes(2)?);
    if magic != "FG" {
        return Err(format!("{id}: not a MAKI file (magic '{magic}')"));
    }
    buf.u16()?; // 1027 in every file seen
    let version = buf.i32()?;
    let classes = read_classes(&mut buf)?;
    let methods = read_methods(&mut buf, &classes)?;
    let mut variables = read_variables(&mut buf, version, &classes)?;
    read_constants(&mut buf, &mut variables)?;
    let raw_bindings = read_bindings(&mut buf, &variables)?;
    let (code, offset_to_index) = read_code(&mut buf)?;
    let mut bindings = Vec::with_capacity(raw_bindings.len());
    for (v, m, off) in raw_bindings {
        let Some(&ci) = offset_to_index.get(&off) else {
            return Err(format!("{id}: binding at bad offset {off}"));
        };
        bindings.push(Binding {
            variable: v,
            method: m,
            code_index: ci,
        });
    }
    Ok(Program {
        id: id.to_string(),
        version,
        classes,
        methods,
        variables,
        bindings,
        code,
    })
}

fn count(buf: &mut Buf, what: &str) -> Result<usize, String> {
    let n = buf.i32()?;
    if !(0..=MAX_TABLE).contains(&n) {
        return Err(format!("{}: {what} count {n} out of range", buf.id));
    }
    Ok(n as usize)
}

fn read_string(buf: &mut Buf) -> Result<String, String> {
    let len = buf.u16()? as usize;
    if len > buf.remaining() {
        return Err(format!("{}: string runs past the end", buf.id));
    }
    Ok(latin1(buf.bytes(len)?))
}

fn read_classes(buf: &mut Buf) -> Result<Vec<String>, String> {
    let n = count(buf, "class")?;
    let mut out = Vec::with_capacity(n);
    for _ in 0..n {
        let d1 = buf.i32()? as u32;
        let d2 = buf.u16()?;
        let d3 = buf.u16()?;
        let tail = buf.bytes(8)?;
        let mut s = format!("{d1:08x}{d2:04x}{d3:04x}");
        for b in tail {
            s.push_str(&format!("{b:02x}"));
        }
        out.push(s);
    }
    Ok(out)
}

fn read_methods(buf: &mut Buf, classes: &[String]) -> Result<Vec<MethodRef>, String> {
    let n = count(buf, "method")?;
    let mut out = Vec::with_capacity(n);
    for _ in 0..n {
        let class_index = (buf.u16()? & 0xff) as usize;
        buf.u16()?;
        let name = read_string(buf)?.to_lowercase();
        if class_index >= classes.len() {
            return Err(format!("{}: method {name} on class {class_index}", buf.id));
        }
        out.push(MethodRef { class_index, name });
    }
    Ok(out)
}

fn read_variables(buf: &mut Buf, version: i32, classes: &[String]) -> Result<Vec<VarDecl>, String> {
    let n = count(buf, "variable")?;
    let mut out: Vec<VarDecl> = Vec::with_capacity(n);
    for _ in 0..n {
        let type_offset = buf.u8()? as usize;
        let is_object = buf.u8()? != 0;
        let subclass = buf.u16()?;
        let u1 = buf.u16()? as i32;
        let u2 = buf.u16()? as i32;
        buf.u16()?;
        buf.u16()?;
        let global = buf.u8()? != 0;
        let system = version > 22 && buf.u8()? != 0;
        let decl = if subclass != 0 {
            let Some(base) = out.get(type_offset) else {
                return Err(format!("{}: subclass of bad var {type_offset}", buf.id));
            };
            VarDecl {
                kind: Kind::Object,
                guid: base.guid.clone(),
                class_var: Some(type_offset),
                global,
                initial: Value::Null,
                system: false,
                constant: None,
            }
        } else if is_object {
            let Some(guid) = classes.get(type_offset) else {
                return Err(format!("{}: object var of bad class {type_offset}", buf.id));
            };
            VarDecl {
                kind: Kind::Object,
                guid: Some(guid.clone()),
                class_var: None,
                global,
                initial: Value::Null,
                system,
                constant: None,
            }
        } else {
            primitive_var(buf.id, type_offset as i32, u1, u2, global)?
        };
        out.push(decl);
    }
    Ok(out)
}

/// A variable of a primitive type; `u1` and `u2` are the low and high halves of its initial value.
fn primitive_var(
    id: &str,
    type_offset: i32,
    u1: i32,
    u2: i32,
    global: bool,
) -> Result<VarDecl, String> {
    let Some(kind) = Kind::of_type(type_offset) else {
        return Err(format!("{id}: primitive type {type_offset}"));
    };
    let initial = match kind {
        Kind::Int => Value::Int((u2 << 16) | u1),
        Kind::Boolean => Value::Bool(u1 != 0),
        Kind::Float | Kind::Double => Value::Num(f32::from_bits(((u2 << 16) | u1) as u32) as f64),
        Kind::String => Value::str(""),
        Kind::Object => Value::Null,
    };
    Ok(VarDecl {
        kind,
        guid: None,
        class_var: None,
        global,
        initial,
        system: false,
        constant: None,
    })
}

fn read_constants(buf: &mut Buf, variables: &mut [VarDecl]) -> Result<(), String> {
    let n = count(buf, "constant")?;
    for _ in 0..n {
        let index = buf.i32()?;
        let value = read_string(buf)?;
        let Some(decl) = usize::try_from(index)
            .ok()
            .and_then(|i| variables.get_mut(i))
        else {
            return Err(format!("{}: constant for bad var {index}", buf.id));
        };
        decl.constant = Some(Value::string(value));
    }
    Ok(())
}

fn read_bindings(buf: &mut Buf, variables: &[VarDecl]) -> Result<Vec<(usize, usize, i32)>, String> {
    let n = count(buf, "binding")?;
    let mut out = Vec::with_capacity(n);
    for _ in 0..n {
        let v = buf.i32()?;
        let m = buf.i32()?;
        let off = buf.i32()?;
        let Some(v) = usize::try_from(v).ok().filter(|i| *i < variables.len()) else {
            return Err(format!("{}: binding on bad var {v}", buf.id));
        };
        let Some(m) = usize::try_from(m).ok() else {
            return Err(format!("{}: binding on bad method {m}", buf.id));
        };
        out.push((v, m, off));
    }
    Ok(out)
}

fn read_code(buf: &mut Buf) -> Result<(Vec<Op>, HashMap<i32, usize>), String> {
    let length = buf.i32()?;
    if length < 0 || length as usize > buf.remaining() {
        return Err(format!("{}: code length {length}", buf.id));
    }
    let start = buf.pos;
    let end = start + length as usize;
    let mut raw = Vec::new();
    let mut offset_to_index = HashMap::new();
    while buf.pos < end {
        let o = read_op(buf, start, end)?;
        offset_to_index.insert(o.offset, raw.len());
        raw.push(o);
    }
    buf.pos = end;
    let mut ops = Vec::with_capacity(raw.len());
    for o in raw {
        let resolved = if op::is_jump(o.opcode) {
            match offset_to_index.get(&o.arg) {
                Some(i) => *i as i32,
                None => {
                    return Err(format!(
                        "{}: jump to bad offset {} from {}",
                        buf.id, o.arg, o.offset
                    ))
                }
            }
        } else {
            o.arg
        };
        ops.push(Op {
            opcode: o.opcode,
            arg: resolved,
            argc: o.argc,
            offset: o.offset,
        });
    }
    Ok((ops, offset_to_index))
}

/// Reads the instruction at the buffer's position in the code section that runs from `start`
/// to `end`. A jump's argument is the byte offset it goes to, which `read_code` turns into a
/// code index; a member access's is the member's type.
fn read_op(buf: &mut Buf, start: usize, end: usize) -> Result<Op, String> {
    let pos = (buf.pos - start) as i32;
    let opcode = buf.u8()?;
    let mut arg = 0;
    let mut argc = -1;
    if op::is_jump(opcode) || op::is_index(opcode) {
        if buf.pos + 4 > end {
            return Err(format!("{}: opcode {opcode} truncated at {pos}", buf.id));
        }
        arg = if op::is_jump(opcode) {
            buf.i32()?.wrapping_add(5).wrapping_add(pos)
        } else {
            buf.i32()?
        };
        if buf.pos + 4 <= end {
            let peek = buf.peek_i32(buf.pos)? as u32;
            if (0xffff0000..=0xffff000f).contains(&peek) {
                buf.i32()?;
                argc = (peek & 0xffff) as i32;
            }
        }
        if opcode == op::CALL2 {
            let count = buf.u8()? as i32;
            if argc < 0 {
                argc = count;
            }
        }
    }
    Ok(Op {
        opcode,
        arg,
        argc,
        offset: pos,
    })
}
