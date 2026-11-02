//! Prettier-style layout: a group stays on one line if it fits in `width`,
//! otherwise it breaks. Objects written multiline in the source stay broken
//! (and force their ancestors to break). Arrays of numbers are filled.

use crate::parser::{Comment, Document, Node, Value};

pub struct Options {
    pub print_width: usize,
    pub indent: String,
}

pub fn format(doc: &Document, opts: &Options) -> String {
    let mut out = String::new();
    for c in &doc.leading {
        out.push_str(&c.text);
        out.push('\n');
    }
    print(&doc.value, opts, 0, 0, 0, &mut out);
    for c in &doc.trailing {
        out.push(if c.nl_before { '\n' } else { ' ' });
        out.push_str(&c.text);
    }
    out.push('\n');
    out
}

fn has_comments(n: &Node) -> bool {
    !n.leading.is_empty() || !n.trailing.is_empty()
}

fn lines(cs: &[Comment], indent: &str, out: &mut String) {
    for c in cs {
        out.push_str(indent);
        out.push_str(&c.text);
        out.push('\n');
    }
}

/// Print leading comments; inline block comments stay on the item's line.
/// Returns true if the item's indent has already been written.
fn leading(cs: &[Comment], indent: &str, out: &mut String) -> bool {
    let mut open = false;
    for c in cs {
        if c.inline {
            if !open {
                out.push_str(indent);
                open = true;
            }
            out.push_str(&c.text);
            out.push(' ');
        } else {
            if open {
                out.push('\n');
                open = false;
            }
            out.push_str(indent);
            out.push_str(&c.text);
            out.push('\n');
        }
    }
    open
}

fn suffix(cs: &[Comment], out: &mut String) {
    for c in cs {
        out.push(' ');
        out.push_str(&c.text);
    }
}

fn forced_break(v: &Value) -> bool {
    match v {
        Value::Object { members, multiline, end } => {
            *multiline
                || !end.is_empty()
                || members.iter().any(|(_, n)| has_comments(n) || forced_break(&n.value))
        }
        Value::Array { items, end } => {
            !end.is_empty() || items.iter().any(|n| has_comments(n) || forced_break(&n.value))
        }
        _ => false,
    }
}

fn flat(v: &Value, out: &mut String) {
    match v {
        Value::Null => out.push_str("null"),
        Value::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        Value::Number(s) | Value::Str(s) => out.push_str(s),
        Value::Array { items, .. } => {
            out.push('[');
            for (i, it) in items.iter().enumerate() {
                if i > 0 {
                    out.push_str(", ");
                }
                flat(&it.value, out);
            }
            out.push(']');
        }
        Value::Object { members, .. } if members.is_empty() => out.push_str("{}"),
        Value::Object { members, .. } => {
            out.push_str("{ ");
            for (i, (k, it)) in members.iter().enumerate() {
                if i > 0 {
                    out.push_str(", ");
                }
                out.push_str(k);
                out.push_str(": ");
                flat(&it.value, out);
            }
            out.push_str(" }");
        }
    }
}

/// `col`: current column where the value starts. `trailing`: width of text
/// that must follow on the same line (a comma). `level`: indent depth.
fn print(v: &Value, o: &Options, level: usize, col: usize, trailing: usize, out: &mut String) {
    let items_empty = match v {
        Value::Array { items, end } => items.is_empty() && end.is_empty(),
        Value::Object { members, end, .. } => members.is_empty() && end.is_empty(),
        _ => true,
    };
    if items_empty {
        flat(v, out);
        return;
    }
    if !forced_break(v) {
        let mut s = String::new();
        flat(v, &mut s);
        if col + s.chars().count() + trailing <= o.print_width {
            out.push_str(&s);
            return;
        }
    }
    let inner = o.indent.repeat(level + 1);
    let inner_col = inner.chars().count();
    let close = o.indent.repeat(level);
    match v {
        Value::Array { items, end }
            if end.is_empty()
                && items.iter().all(|n| matches!(n.value, Value::Number(_)) && !has_comments(n)) =>
        {
            // fill: pack as many numbers per line as fit
            out.push_str("[\n");
            out.push_str(&inner);
            let mut c = inner_col;
            for (i, it) in items.iter().enumerate() {
                let Value::Number(n) = &it.value else { unreachable!() };
                let last = i + 1 == items.len();
                let w = n.len() + if last { 0 } else { 1 };
                if i > 0 {
                    if c + 1 + w <= o.print_width {
                        out.push(' ');
                        c += 1;
                    } else {
                        out.push('\n');
                        out.push_str(&inner);
                        c = inner_col;
                    }
                }
                out.push_str(n);
                c += n.len();
                if !last {
                    out.push(',');
                    c += 1;
                }
            }
            out.push('\n');
            out.push_str(&close);
            out.push(']');
        }
        Value::Array { items, end } => {
            out.push_str("[\n");
            for (i, it) in items.iter().enumerate() {
                let last = i + 1 == items.len();
                if !leading(&it.leading, &inner, out) {
                    out.push_str(&inner);
                }
                print(&it.value, o, level + 1, inner_col, usize::from(!last), out);
                if !last {
                    out.push(',');
                }
                suffix(&it.trailing, out);
                out.push('\n');
            }
            lines(end, &inner, out);
            out.push_str(&close);
            out.push(']');
        }
        Value::Object { members, end, .. } => {
            out.push_str("{\n");
            for (i, (k, it)) in members.iter().enumerate() {
                let last = i + 1 == members.len();
                if !leading(&it.leading, &inner, out) {
                    out.push_str(&inner);
                }
                out.push_str(k);
                out.push_str(": ");
                let c = inner_col + k.chars().count() + 2;
                print(&it.value, o, level + 1, c, usize::from(!last), out);
                if !last {
                    out.push(',');
                }
                suffix(&it.trailing, out);
                out.push('\n');
            }
            lines(end, &inner, out);
            out.push_str(&close);
            out.push('}');
        }
        _ => unreachable!(),
    }
}
