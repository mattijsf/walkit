// SPDX-License-Identifier: GPL-3.0-or-later

//! A MAKI value and the handles scripts hold objects by.

use std::rc::Rc;

/// An object a script can hold: one of the skin's GUI objects, a container, a script's System
/// object, or an object a script made (a timer, a list, ...), each by its index in the runtime.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Handle {
    /// A GUI object, by its index in the runtime's objects.
    Gui(usize),
    /// A container, by its index.
    Container(usize),
    /// A script's System object, by its index.
    System(usize),
    /// An object a script made with `new`, by its index.
    Made(usize),
}

/// A MAKI value. Integers and floats are kept apart because assignment to an Int variable truncates.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    /// A 32-bit integer.
    Int(i32),
    /// A double.
    Num(f64),
    /// A boolean.
    Bool(bool),
    /// A string.
    Str(Rc<str>),
    /// An object.
    Obj(Handle),
    /// Nothing.
    Null,
}

impl Value {
    /// A string value.
    pub fn str(s: &str) -> Value {
        Value::Str(Rc::from(s))
    }

    /// A string value from an owned string.
    pub fn string(s: String) -> Value {
        Value::Str(Rc::from(s))
    }

    /// An object, or `Null` for none.
    pub fn of(h: Option<Handle>) -> Value {
        match h {
            Some(h) => Value::Obj(h),
            None => Value::Null,
        }
    }

    /// Whether it counts as true.
    pub fn truthy(&self) -> bool {
        match self {
            Value::Int(v) => *v != 0,
            Value::Num(v) => *v != 0.0,
            Value::Bool(v) => *v,
            Value::Str(s) => !s.is_empty(),
            Value::Obj(_) => true,
            Value::Null => false,
        }
    }

    /// As a double.
    pub fn as_double(&self) -> f64 {
        match self {
            Value::Int(v) => *v as f64,
            Value::Num(v) => *v,
            Value::Bool(v) => {
                if *v {
                    1.0
                } else {
                    0.0
                }
            }
            Value::Str(s) => parse_double(s.trim()).unwrap_or(0.0),
            _ => 0.0,
        }
    }

    /// As an integer: a double truncated, a string parsed.
    pub fn as_int(&self) -> i32 {
        match self {
            Value::Int(v) => *v,
            Value::Num(v) => double_to_int(*v),
            Value::Bool(v) => *v as i32,
            Value::Str(s) => {
                let t = s.trim();
                t.parse::<i32>()
                    .ok()
                    .or_else(|| parse_double(t).map(double_to_int))
                    .unwrap_or(0)
            }
            _ => 0,
        }
    }

    /// As a string: a whole double without its fraction.
    pub fn as_string(&self) -> Rc<str> {
        match self {
            Value::Str(s) => s.clone(),
            Value::Int(v) => Rc::from(v.to_string()),
            Value::Num(v) => Rc::from(format_double(*v)),
            Value::Bool(v) => Rc::from(if *v { "1" } else { "0" }),
            Value::Obj(h) => Rc::from(format!("Obj({h:?})")),
            Value::Null => Rc::from(""),
        }
    }

    /// The object, if it is one.
    pub fn as_object(&self) -> Option<Handle> {
        match self {
            Value::Obj(h) => Some(*h),
            _ => None,
        }
    }

    /// Whether it is an Int, a Num or a Bool.
    pub fn is_number(&self) -> bool {
        matches!(self, Value::Int(_) | Value::Num(_) | Value::Bool(_))
    }
}

/// A double as Kotlin's `toString` writes it when it is whole and small, else with its fraction.
pub fn format_double(v: f64) -> String {
    if v == v.floor() && v.abs() < 1e15 {
        (v as i64).to_string()
    } else {
        kotlin_double(v)
    }
}

/// A double as Kotlin writes it: the shortest digits that read back, with an exponent outside
/// 1e-3..1e7, and always a fraction.
pub fn kotlin_double(v: f64) -> String {
    if v.is_nan() {
        return "NaN".into();
    }
    if v.is_infinite() {
        return if v > 0.0 {
            "Infinity".into()
        } else {
            "-Infinity".into()
        };
    }
    let a = v.abs();
    if a != 0.0 && !(1e-3..1e7).contains(&a) {
        let s = format!("{v:e}");
        let (m, e) = s.split_once('e').unwrap_or((&s, "0"));
        let m = if m.contains('.') {
            m.to_string()
        } else {
            format!("{m}.0")
        };
        return format!("{m}E{e}");
    }
    let s = format!("{v}");
    if s.contains('.') {
        s
    } else {
        format!("{s}.0")
    }
}

/// A string read as a double the way Kotlin's `toDoubleOrNull` reads it.
pub fn parse_double(s: &str) -> Option<f64> {
    if s.is_empty() {
        return None;
    }
    let body = s.trim_end_matches(['d', 'D', 'f', 'F']);
    let core = body.strip_prefix(['+', '-']).unwrap_or(body);
    if core.eq_ignore_ascii_case("nan") || core.eq_ignore_ascii_case("infinity") {
        return body.parse::<f64>().ok();
    }
    if !core.bytes().all(|b| {
        b.is_ascii_digit() || b == b'.' || b == b'e' || b == b'E' || b == b'-' || b == b'+'
    }) {
        return None;
    }
    if core.starts_with('.') && core.len() == 1 {
        return None;
    }
    body.parse::<f64>().ok()
}

/// A double to an int as the JVM does it: truncated, saturated, NaN to 0.
pub fn double_to_int(v: f64) -> i32 {
    if v.is_nan() {
        0
    } else {
        v as i32
    }
}

/// The kind of a variable slot, as the compiler numbers types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Kind {
    /// An integer.
    Int,
    /// A float.
    Float,
    /// A double.
    Double,
    /// A boolean.
    Boolean,
    /// A string.
    String,
    /// An object.
    Object,
}

impl Kind {
    /// The primitive kind a type number stands for, as the variables table numbers types; `None` for any other.
    pub fn of_type(t: i32) -> Option<Kind> {
        match t {
            2 => Some(Kind::Int),
            3 => Some(Kind::Float),
            4 => Some(Kind::Double),
            5 => Some(Kind::Boolean),
            6 => Some(Kind::String),
            _ => None,
        }
    }

    /// The kind a value is.
    pub fn of(v: &Value) -> Kind {
        match v {
            Value::Int(_) => Kind::Int,
            Value::Num(_) => Kind::Double,
            Value::Bool(_) => Kind::Boolean,
            Value::Str(_) => Kind::String,
            _ => Kind::Object,
        }
    }

    /// `v` coerced to this kind, as assignment to a declared variable does.
    pub fn coerce(self, v: Value) -> Value {
        match self {
            Kind::Int => match v {
                Value::Int(_) => v,
                _ => Value::Int(v.as_int()),
            },
            Kind::Float | Kind::Double => match v {
                Value::Num(_) => v,
                _ => Value::Num(v.as_double()),
            },
            Kind::Boolean => match v {
                Value::Bool(_) => v,
                _ => Value::Bool(v.truthy()),
            },
            Kind::String => match v {
                Value::Str(_) => v,
                _ => Value::Str(v.as_string()),
            },
            Kind::Object => match v {
                Value::Obj(_) | Value::Null => v,
                _ => Value::Null,
            },
        }
    }
}

/// A variable slot. Assignment coerces to the declared kind, as the compiler assumes.
#[derive(Debug, Clone)]
pub struct Cell {
    /// The declared kind.
    pub kind: Kind,
    /// The value held.
    pub value: Value,
}

impl Cell {
    /// A slot of `kind` holding `initial` as given.
    pub fn new(kind: Kind, initial: Value) -> Cell {
        Cell {
            kind,
            value: initial,
        }
    }

    /// Assigns `v`, coerced to the slot's kind.
    pub fn assign(&mut self, v: Value) {
        self.value = self.kind.coerce(v);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strings_read_as_numbers() {
        assert_eq!(Value::str(" 12 ").as_int(), 12);
        assert_eq!(Value::str("1.9").as_int(), 1);
        assert_eq!(Value::str("x").as_int(), 0);
        assert_eq!(Value::Num(3.0).as_string().as_ref(), "3");
        assert_eq!(Value::Num(2.5).as_string().as_ref(), "2.5");
        assert_eq!(kotlin_double(1e7), "1.0E7");
        assert_eq!(kotlin_double(0.5), "0.5");
        assert_eq!(kotlin_double(100.0), "100.0");
    }
}
