// SPDX-License-Identifier: GPL-3.0-or-later

//! A compiled MAKI script, read from a `.maki` file. See the parser for the layout.

use crate::maki::{Kind, Value};

/// A compiled MAKI script.
#[derive(Debug, Clone)]
pub struct Program {
    /// The script's file name, for messages.
    pub id: String,
    /// The format version.
    pub version: i32,
    /// Class GUIDs, lowercase hex without dashes, in the order the file declares them.
    pub classes: Vec<String>,
    /// The methods the script imports.
    pub methods: Vec<MethodRef>,
    /// The variables.
    pub variables: Vec<VarDecl>,
    /// The event bindings.
    pub bindings: Vec<Binding>,
    /// The code.
    pub code: Vec<Op>,
}

impl Program {
    /// The name of class `index`, as the library names it, or its GUID when the library does not know it.
    pub fn class_name(&self, index: usize) -> String {
        self.classes
            .get(index)
            .map(|g| {
                crate::maki::stdapi::class(g)
                    .map(|c| c.name.to_string())
                    .unwrap_or_else(|| g.clone())
            })
            .unwrap_or_default()
    }
}

/// A method the script imports: on which class, and its lowercase name.
#[derive(Debug, Clone)]
pub struct MethodRef {
    /// The class index.
    pub class_index: usize,
    /// The method name, lowercase.
    pub name: String,
}

/// A variable the script declares.
#[derive(Debug, Clone)]
pub struct VarDecl {
    /// Its kind.
    pub kind: Kind,
    /// For objects: the class GUID.
    pub guid: Option<String>,
    /// For a variable declared with `Class X Y`: the index of the class variable it belongs to.
    pub class_var: Option<usize>,
    /// Declared global.
    pub global: bool,
    /// The initial value.
    pub initial: Value,
    /// A predeclared global the runtime fills in: System, Config, `PlDir` and the like.
    pub system: bool,
    /// A string literal bound to this variable by the constants section.
    pub constant: Option<Value>,
}

/// An event handler: on the object in `variable`, for the event `method` names, starting at `code_index`.
#[derive(Debug, Clone)]
pub struct Binding {
    /// The variable holding the object.
    pub variable: usize,
    /// The method (event) index.
    pub method: usize,
    /// Where the handler starts.
    pub code_index: usize,
}

/// One instruction.
#[derive(Debug, Clone, Copy)]
pub struct Op {
    /// The opcode.
    pub opcode: u8,
    /// Variable, method or class index, a member's type, or a code index for jumps (a byte
    /// offset while the parser reads the code).
    pub arg: i32,
    /// The argument count carried by a stack-protected call, or -1.
    pub argc: i32,
    /// Byte offset in the code section, for messages.
    pub offset: i32,
}

/// The opcodes.
pub mod op {
    /// Push a variable.
    pub const PUSH: u8 = 0x01;
    /// Drop the top.
    pub const POP: u8 = 0x02;
    /// Pop into a variable.
    pub const POP_TO: u8 = 0x03;
    /// Equal.
    pub const EQ: u8 = 0x08;
    /// Not equal.
    pub const NE: u8 = 0x09;
    /// Greater.
    pub const GT: u8 = 0x0a;
    /// Greater or equal.
    pub const GE: u8 = 0x0b;
    /// Less.
    pub const LT: u8 = 0x0c;
    /// Less or equal.
    pub const LE: u8 = 0x0d;
    /// Jump unless the top is true.
    pub const JUMP_IF_NOT: u8 = 0x10;
    /// Jump when the top is true.
    pub const JUMP_IF: u8 = 0x11;
    /// Jump.
    pub const JUMP: u8 = 0x12;
    /// Call a method.
    pub const CALL: u8 = 0x18;
    /// Call a function of the script.
    pub const CALL_GLOBAL: u8 = 0x19;
    /// Return.
    pub const RETURN: u8 = 0x21;
    /// The end of a handler's prologue.
    pub const COMPLETE: u8 = 0x28;
    /// Assign.
    pub const MOV: u8 = 0x30;
    /// Post-increment.
    pub const POST_INC: u8 = 0x38;
    /// Post-decrement.
    pub const POST_DEC: u8 = 0x39;
    /// Pre-increment.
    pub const PRE_INC: u8 = 0x3a;
    /// Pre-decrement.
    pub const PRE_DEC: u8 = 0x3b;
    /// Add.
    pub const ADD: u8 = 0x40;
    /// Subtract.
    pub const SUB: u8 = 0x41;
    /// Multiply.
    pub const MUL: u8 = 0x42;
    /// Divide.
    pub const DIV: u8 = 0x43;
    /// Modulo.
    pub const MOD: u8 = 0x44;
    /// Bitwise and.
    pub const AND: u8 = 0x48;
    /// Bitwise or.
    pub const OR: u8 = 0x49;
    /// Logical not.
    pub const NOT: u8 = 0x4a;
    /// Bitwise not.
    pub const BIT_NOT: u8 = 0x4b;
    /// Negate.
    pub const NEG: u8 = 0x4c;
    /// Bitwise xor.
    pub const XOR: u8 = 0x4d;
    /// Logical and.
    pub const LOG_AND: u8 = 0x50;
    /// Logical or.
    pub const LOG_OR: u8 = 0x51;
    /// Shift left.
    pub const SHL: u8 = 0x58;
    /// Shift right.
    pub const SHR: u8 = 0x59;
    /// Shift left, the other encoding.
    pub const SHL2: u8 = 0x5a;
    /// Shift right, the other encoding.
    pub const SHR2: u8 = 0x5b;
    /// Make an object.
    pub const NEW: u8 = 0x60;
    /// Delete an object.
    pub const DELETE: u8 = 0x61;
    /// `object.name` for a variable a script keeps on each object of a class it declares: pops
    /// the name and the object, leaves the variable's slot. The immediate is the variable's type.
    pub const MEMBER: u8 = 0x68;
    /// Call a method, the other encoding.
    pub const CALL2: u8 = 0x70;

    /// Whether the opcode's argument is a jump target.
    pub fn is_jump(o: u8) -> bool {
        matches!(o, JUMP_IF_NOT | JUMP_IF | JUMP | CALL_GLOBAL)
    }

    /// Whether the opcode carries an index argument.
    pub fn is_index(o: u8) -> bool {
        matches!(o, PUSH | POP_TO | CALL | CALL2 | NEW | MEMBER)
    }
}
