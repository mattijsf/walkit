// SPDX-License-Identifier: GPL-3.0-or-later

//! The MAKI standard library: class GUIDs, their parents and each method's arity and return kind.

use std::collections::HashMap;
use std::sync::OnceLock;

#[path = "stdapi_table.rs"]
mod table;

/// One method of a class: its parameter count and return kind (I int, F float, D double, B
/// boolean, S string, V void, O object, A any).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ApiMethod {
    /// How many parameters it takes.
    pub params: u8,
    /// What it returns.
    pub ret: u8,
}

/// One class of the library.
pub struct ApiClass {
    /// Its name, as `getClassName` answers it.
    pub name: &'static str,
    /// Its parent's name.
    pub parent: Option<&'static str>,
    /// Its methods by lowercase name.
    pub methods: HashMap<&'static str, ApiMethod>,
}

struct Tables {
    by_guid: HashMap<&'static str, ApiClass>,
    guid_by_name: HashMap<String, &'static str>,
}

fn tables() -> &'static Tables {
    static T: OnceLock<Tables> = OnceLock::new();
    T.get_or_init(|| {
        let mut by_guid = HashMap::new();
        let mut guid_by_name = HashMap::new();
        for (guid, name, parent, methods) in table::CLASSES {
            let m = methods
                .iter()
                .map(|(n, p, r)| {
                    (
                        *n,
                        ApiMethod {
                            params: *p,
                            ret: *r,
                        },
                    )
                })
                .collect();
            by_guid.insert(
                *guid,
                ApiClass {
                    name,
                    parent: *parent,
                    methods: m,
                },
            );
            guid_by_name.insert(name.to_lowercase(), *guid);
        }
        Tables {
            by_guid,
            guid_by_name,
        }
    })
}

/// The class with `guid` (lowercase hex without dashes).
pub fn class(guid: &str) -> Option<&'static ApiClass> {
    let t = tables();
    t.by_guid
        .get(guid)
        .or_else(|| t.by_guid.get(guid.to_lowercase().as_str()))
}

/// The GUID of the class named `name`, any case.
pub fn guid_of(name: &str) -> Option<&'static str> {
    tables().guid_by_name.get(&name.to_lowercase()).copied()
}

fn parent_of(c: &ApiClass) -> Option<&'static ApiClass> {
    let t = tables();
    c.parent
        .and_then(|p| t.guid_by_name.get(&p.to_lowercase()))
        .and_then(|g| t.by_guid.get(g))
}

/// Looks a method up on a class or any ancestor.
pub fn method(guid: &str, name: &str) -> Option<ApiMethod> {
    let lower = name.to_lowercase();
    let mut cls = class(guid);
    while let Some(c) = cls {
        if let Some(m) = c.methods.get(lower.as_str()) {
            return Some(*m);
        }
        cls = parent_of(c);
    }
    None
}

/// Whether the class with `guid` is `ancestor` or descends from it.
pub fn is_a(guid: &str, ancestor: &str) -> bool {
    let mut cls = class(guid);
    while let Some(c) = cls {
        if c.name.eq_ignore_ascii_case(ancestor) {
            return true;
        }
        cls = parent_of(c);
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn methods_are_found_up_the_chain() {
        let layer = guid_of("Layer").unwrap();
        assert_eq!(
            method(layer, "getLeft"),
            Some(ApiMethod {
                params: 0,
                ret: b'I'
            })
        );
        assert!(is_a(layer, "GuiObject"));
        assert!(!is_a(layer, "Group"));
        assert_eq!(class(guid_of("System").unwrap()).unwrap().name, "System");
    }
}
