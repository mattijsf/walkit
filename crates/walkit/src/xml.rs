// SPDX-License-Identifier: GPL-3.0-or-later

//! A forgiving XML reader for skin files.
//!
//! Skin XML is often not well formed: several roots in one included file, unquoted or repeated
//! attributes, stray `&`, and a byte-order mark. Every file is read as one list of top-level
//! nodes. Unknown entities are left as they are, an unclosed element ends at its parent's close
//! tag, and a close tag that matches nothing is dropped.

use std::rc::Rc;

/// An element of a skin XML file. Names and attribute keys are lowercase; values are as written.
#[derive(Debug, Clone)]
pub struct XmlNode {
    /// The tag name, lowercase.
    pub name: String,
    /// The attributes in the order written; a key written twice keeps its first value.
    pub attrs: Vec<(String, String)>,
    /// The child elements, in order.
    pub children: Vec<Rc<XmlNode>>,
    /// The element's own text, between its tags and outside its children, trimmed: `<name>Bento</name>`.
    pub text: String,
}

impl XmlNode {
    /// The value of the attribute `key` (lowercase), if written.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.attrs
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    }

    /// Whether the attribute `key` is written.
    pub fn has(&self, key: &str) -> bool {
        self.attrs.iter().any(|(k, _)| k == key)
    }

    /// A node with other attributes, the same children and text.
    #[must_use]
    pub fn with_attrs(&self, attrs: Vec<(String, String)>) -> XmlNode {
        XmlNode {
            name: self.name.clone(),
            attrs,
            children: self.children.clone(),
            text: self.text.clone(),
        }
    }
}

/// Every top-level element of `text`.
pub fn parse(text: &str) -> Vec<Rc<XmlNode>> {
    let text = text.strip_prefix('\u{FEFF}').unwrap_or(text);
    Reader {
        s: text.as_bytes(),
        src: text,
        i: 0,
        stack: Vec::new(),
        roots: Vec::new(),
    }
    .parse_all()
}

struct Open {
    name: String,
    attrs: Vec<(String, String)>,
    children: Vec<Rc<XmlNode>>,
    text: Option<String>,
}

impl Open {
    fn add_text(&mut self, part: &str) {
        if part.trim().is_empty() {
            return;
        }
        self.text.get_or_insert_with(String::new).push_str(part);
    }
}

struct Reader<'a> {
    s: &'a [u8],
    src: &'a str,
    i: usize,
    stack: Vec<Open>,
    roots: Vec<Rc<XmlNode>>,
}

impl Reader<'_> {
    fn parse_all(mut self) -> Vec<Rc<XmlNode>> {
        while self.i < self.s.len() {
            let lt = self.find(b"<", self.i);
            let end = lt.unwrap_or(self.s.len());
            let part = unescape(&self.src[self.i..end]);
            if let Some(top) = self.stack.last_mut() {
                top.add_text(&part);
            }
            let Some(lt) = lt else { break };
            self.i = lt + 1;
            if self.starts_with(b"!--") {
                self.skip_past(b"-->");
            } else if self.starts_with(b"![CDATA[") {
                let start = self.i + "![CDATA[".len();
                self.skip_past(b"]]>");
                let stop = start.max(self.i.saturating_sub(3));
                let part = self.src[start..stop].to_string();
                if let Some(top) = self.stack.last_mut() {
                    top.add_text(&part);
                }
            } else if self.starts_with(b"!") || self.starts_with(b"?") {
                self.skip_past(b">");
            } else if self.starts_with(b"/") {
                self.i += 1;
                let name = self.read_name().to_lowercase();
                self.skip_past(b">");
                self.close(&name);
            } else {
                self.open_tag();
            }
        }
        while let Some(open) = self.stack.pop() {
            self.finish(open);
        }
        self.roots
    }

    fn find(&self, pat: &[u8], from: usize) -> Option<usize> {
        if from >= self.s.len() {
            return None;
        }
        self.s[from..]
            .windows(pat.len())
            .position(|w| w == pat)
            .map(|p| p + from)
    }

    fn starts_with(&self, pat: &[u8]) -> bool {
        self.s[self.i..].starts_with(pat)
    }

    fn skip_past(&mut self, end: &[u8]) {
        self.i = match self.find(end, self.i) {
            Some(at) => at + end.len(),
            None => self.s.len(),
        };
    }

    fn read_name(&mut self) -> &str {
        let start = self.i;
        while self.i < self.s.len() && !ends_name(self.s[self.i]) {
            self.i += 1;
        }
        &self.src[start..self.i]
    }

    fn skip_space(&mut self) {
        while self.i < self.s.len() && self.s[self.i].is_ascii_whitespace() {
            self.i += 1;
        }
    }

    fn open_tag(&mut self) {
        let name = self.read_name().to_lowercase();
        let mut attrs: Vec<(String, String)> = Vec::new();
        let mut self_close = false;
        while self.i < self.s.len() {
            self.skip_space();
            if self.i >= self.s.len() {
                break;
            }
            match self.s[self.i] {
                b'>' => {
                    self.i += 1;
                    break;
                }
                b'/' => {
                    self_close = true;
                    self.i += 1;
                }
                b'<' => break,
                _ => self.read_attribute(&mut attrs),
            }
        }
        let open = Open {
            name,
            attrs,
            children: Vec::new(),
            text: None,
        };
        if self_close {
            self.finish(open);
        } else {
            self.stack.push(open);
        }
    }

    /// Reads one attribute into `attrs`; a key that is already there keeps its first value.
    fn read_attribute(&mut self, attrs: &mut Vec<(String, String)>) {
        let key = self.read_name().to_lowercase();
        if key.is_empty() {
            self.i += 1;
            return;
        }
        self.skip_space();
        let mut value = String::new();
        if self.i < self.s.len() && self.s[self.i] == b'=' {
            self.i += 1;
            self.skip_space();
            value = self.read_value();
        }
        if !attrs.iter().any(|(k, _)| *k == key) {
            attrs.push((key, unescape(&value)));
        }
    }

    /// Reads an attribute value: quoted, or unquoted up to a space, `>` or `/>`.
    fn read_value(&mut self) -> String {
        if self.i < self.s.len() && (self.s[self.i] == b'"' || self.s[self.i] == b'\'') {
            let q = self.s[self.i];
            let end = self.find(&[q], self.i + 1).unwrap_or(self.s.len());
            let v = self.src[self.i + 1..end].to_string();
            self.i = (end + 1).min(self.s.len());
            v
        } else {
            let start = self.i;
            while self.i < self.s.len() && !self.ends_unquoted_value(self.i) {
                self.i += 1;
            }
            self.src[start..self.i].to_string()
        }
    }

    fn ends_unquoted_value(&self, at: usize) -> bool {
        let c = self.s[at];
        c.is_ascii_whitespace() || c == b'>' || (c == b'/' && self.s.get(at + 1) == Some(&b'>'))
    }

    fn close(&mut self, name: &str) {
        let Some(at) = self.stack.iter().rposition(|o| o.name == name) else {
            return;
        };
        while self.stack.len() > at {
            let open = self.stack.pop().expect("stack holds at");
            self.finish(open);
        }
    }

    fn finish(&mut self, open: Open) {
        let node = Rc::new(XmlNode {
            name: open.name,
            attrs: open.attrs,
            children: open.children,
            text: open.text.map(|t| t.trim().to_string()).unwrap_or_default(),
        });
        match self.stack.last_mut() {
            Some(top) => top.children.push(node),
            None => self.roots.push(node),
        }
    }
}

/// Whether a tag or attribute name stops at `c`.
fn ends_name(c: u8) -> bool {
    c.is_ascii_whitespace() || c == b'>' || c == b'/' || c == b'='
}

fn unescape(v: &str) -> String {
    if !v.contains('&') {
        return v.to_string();
    }
    v.replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn forgiving_reads() {
        let nodes = parse("\u{FEFF}<?xml version=\"1.0\"?><!-- c --><A x=1 Y='2' x=3><b/><c>Text &amp; more</c></A><d>");
        assert_eq!(nodes.len(), 2);
        let a = &nodes[0];
        assert_eq!(a.name, "a");
        assert_eq!(a.get("x"), Some("1"));
        assert_eq!(a.get("y"), Some("2"));
        assert_eq!(a.children.len(), 2);
        assert_eq!(a.children[1].text, "Text & more");
        assert_eq!(nodes[1].name, "d");
    }

    #[test]
    fn a_stray_close_tag_is_dropped_and_an_unclosed_element_ends_with_its_parent() {
        let nodes = parse("<a><b></x></a>");
        assert_eq!(nodes.len(), 1);
        assert_eq!(nodes[0].children.len(), 1);
        assert_eq!(nodes[0].children[0].name, "b");
    }

    #[test]
    fn unquoted_values_stop_before_a_self_close() {
        let nodes = parse("<a file=x.png/><b/>");
        assert_eq!(nodes[0].get("file"), Some("x.png"));
        assert_eq!(nodes.len(), 2);
    }
}
