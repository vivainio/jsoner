//! Minimal JSON parser that keeps raw number/string text and remembers
//! whether an object was written across multiple lines (prettier preserves that).

#[derive(Debug, Clone)]
pub enum Value {
    Null,
    Bool(bool),
    /// Raw number text, kept verbatim.
    Number(String),
    /// Raw string text including quotes and escapes.
    Str(String),
    Array {
        items: Vec<Node>,
        /// Comments after the last item (or inside an empty array).
        end: Vec<Comment>,
    },
    Object {
        members: Vec<(String, Node)>,
        /// True if there was a newline between `{` and the first key.
        multiline: bool,
        /// Comments after the last member (or inside an empty object).
        end: Vec<Comment>,
    },
}

#[derive(Debug, Clone)]
pub struct Comment {
    /// Raw text including `//` or `/* */`.
    pub text: String,
    /// A newline separates this comment from the preceding token/comment.
    pub nl_before: bool,
    /// Block comment followed by more code on the same line.
    pub inline: bool,
}

/// A value plus the comments attached to it.
#[derive(Debug, Clone)]
pub struct Node {
    /// Comments on their own lines before the item.
    pub leading: Vec<Comment>,
    /// Comments on the same line after the item (and its comma).
    pub trailing: Vec<Comment>,
    pub value: Value,
}

#[derive(Debug, Clone)]
pub struct Document {
    pub leading: Vec<Comment>,
    pub value: Value,
    pub trailing: Vec<Comment>,
}

pub struct ParseError {
    pub line: usize,
    pub col: usize,
    pub msg: String,
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}: {}", self.line, self.col, self.msg)
    }
}

pub fn parse(src: &str) -> Result<Document, ParseError> {
    let mut p = Parser { s: src.as_bytes(), src, i: 0, depth: 0 };
    let leading = p.trivia()?;
    let value = p.value()?;
    let trailing = p.trivia()?;
    if p.i < p.s.len() {
        return Err(p.err("unexpected trailing content"));
    }
    Ok(Document { leading, value, trailing })
}

struct Parser<'a> {
    s: &'a [u8],
    src: &'a str,
    i: usize,
    depth: usize,
}

const MAX_DEPTH: usize = 512;

impl<'a> Parser<'a> {
    fn err(&self, msg: &str) -> ParseError {
        let before = &self.src[..self.i.min(self.src.len())];
        let line = before.matches('\n').count() + 1;
        let col = before.rsplit('\n').next().map_or(0, |l| l.chars().count()) + 1;
        ParseError { line, col, msg: msg.to_string() }
    }

    fn peek(&self) -> Option<u8> {
        self.s.get(self.i).copied()
    }

    /// Skip whitespace and comments, returning the comments found.
    fn trivia(&mut self) -> Result<Vec<Comment>, ParseError> {
        let mut out = Vec::new();
        let mut nl = false;
        loop {
            match self.peek() {
                Some(b'\n') => {
                    nl = true;
                    self.i += 1;
                }
                Some(b' ' | b'\t' | b'\r') => self.i += 1,
                Some(b'/') => {
                    let start = self.i;
                    match self.s.get(self.i + 1) {
                        Some(b'/') => {
                            while matches!(self.peek(), Some(c) if c != b'\n') {
                                self.i += 1;
                            }
                            let text = self.src[start..self.i].trim_end().to_string();
                            out.push(Comment { text, nl_before: nl, inline: false });
                            nl = true;
                        }
                        Some(b'*') => {
                            self.i += 2;
                            loop {
                                if self.i + 1 >= self.s.len() {
                                    self.i = start;
                                    return Err(self.err("unterminated block comment"));
                                }
                                if self.s[self.i] == b'*' && self.s[self.i + 1] == b'/' {
                                    self.i += 2;
                                    break;
                                }
                                self.i += 1;
                            }
                            let text = self.src[start..self.i].to_string();
                            let inline = self.s[self.i..]
                                .iter()
                                .find(|c| !matches!(c, b' ' | b'\t' | b'\r'))
                                .is_some_and(|c| *c != b'\n');
                            out.push(Comment { text, nl_before: nl, inline });
                            nl = false;
                        }
                        _ => return Err(self.err("unexpected character '/'")),
                    }
                }
                _ => return Ok(out),
            }
        }
    }

    fn value(&mut self) -> Result<Value, ParseError> {
        match self.peek() {
            None => Err(self.err("unexpected end of input")),
            Some(b'{') => self.object(),
            Some(b'[') => self.array(),
            Some(b'"') => Ok(Value::Str(self.string()?)),
            Some(b't') => self.lit("true", Value::Bool(true)),
            Some(b'f') => self.lit("false", Value::Bool(false)),
            Some(b'n') => self.lit("null", Value::Null),
            Some(b'-' | b'0'..=b'9') => self.number(),
            Some(c) => Err(self.err(&format!("unexpected character '{}'", c as char))),
        }
    }

    fn lit(&mut self, word: &str, v: Value) -> Result<Value, ParseError> {
        if self.s[self.i..].starts_with(word.as_bytes()) {
            self.i += word.len();
            Ok(v)
        } else {
            Err(self.err("invalid literal"))
        }
    }

    fn number(&mut self) -> Result<Value, ParseError> {
        let start = self.i;
        if self.peek() == Some(b'-') {
            self.i += 1;
        }
        let digits = |p: &mut Self| {
            let st = p.i;
            while matches!(p.peek(), Some(b'0'..=b'9')) {
                p.i += 1;
            }
            p.i > st
        };
        if !digits(self) {
            return Err(self.err("invalid number"));
        }
        if self.peek() == Some(b'.') {
            self.i += 1;
            if !digits(self) {
                return Err(self.err("invalid number: expected digits after '.'"));
            }
        }
        if matches!(self.peek(), Some(b'e' | b'E')) {
            self.i += 1;
            if matches!(self.peek(), Some(b'+' | b'-')) {
                self.i += 1;
            }
            if !digits(self) {
                return Err(self.err("invalid number: expected exponent digits"));
            }
        }
        Ok(Value::Number(self.src[start..self.i].to_string()))
    }

    fn string(&mut self) -> Result<String, ParseError> {
        let start = self.i;
        self.i += 1; // opening quote
        loop {
            match self.peek() {
                None => return Err(self.err("unterminated string")),
                Some(b'"') => {
                    self.i += 1;
                    return Ok(self.src[start..self.i].to_string());
                }
                Some(b'\\') => self.i += 2,
                Some(c) if c < 0x20 => return Err(self.err("control character in string")),
                Some(_) => self.i += 1,
            }
        }
    }

    fn enter(&mut self) -> Result<(), ParseError> {
        self.depth += 1;
        if self.depth > MAX_DEPTH {
            return Err(self.err("nesting too deep"));
        }
        Ok(())
    }

    /// After an item: gather comments around the optional comma. Returns
    /// (same-line trailing comments, comments belonging to what follows).
    fn after_item(&mut self, close: u8) -> Result<(Vec<Comment>, Vec<Comment>), ParseError> {
        let mut after = self.trivia()?;
        match self.peek() {
            Some(b',') => {
                self.i += 1;
                after.extend(self.trivia()?);
            }
            Some(c) if c == close => {}
            _ => {
                return Err(self.err(&format!("expected ',' or '{}'", close as char)));
            }
        }
        let k = after.iter().take_while(|c| !c.nl_before).count();
        let mut rest = after.split_off(k);
        // `a, /* c */ b`: a block comment followed by more code leads what follows.
        while rest.is_empty() && self.peek() != Some(close) {
            match after.last() {
                Some(c) if c.inline => rest.insert(0, after.pop().unwrap()),
                _ => break,
            }
        }
        Ok((after, rest))
    }

    fn array(&mut self) -> Result<Value, ParseError> {
        self.enter()?;
        self.i += 1;
        let mut items = Vec::new();
        let mut pending = self.trivia()?;
        loop {
            if self.peek() == Some(b']') {
                self.i += 1;
                break;
            }
            let leading = std::mem::take(&mut pending);
            let value = self.value()?;
            let (trailing, rest) = self.after_item(b']')?;
            pending = rest;
            items.push(Node { leading, trailing, value });
        }
        self.depth -= 1;
        Ok(Value::Array { items, end: pending })
    }

    fn object(&mut self) -> Result<Value, ParseError> {
        self.enter()?;
        self.i += 1;
        let after_brace = self.i;
        let mut members = Vec::new();
        let mut pending = self.trivia()?;
        let multiline = self.s[after_brace..self.i].contains(&b'\n');
        loop {
            if self.peek() == Some(b'}') {
                self.i += 1;
                break;
            }
            if self.peek() != Some(b'"') {
                return Err(self.err("expected string key"));
            }
            let mut leading = std::mem::take(&mut pending);
            let k = self.string()?;
            leading.extend(self.trivia()?);
            if self.peek() != Some(b':') {
                return Err(self.err("expected ':'"));
            }
            self.i += 1;
            leading.extend(self.trivia()?);
            let value = self.value()?;
            let (trailing, rest) = self.after_item(b'}')?;
            pending = rest;
            members.push((k, Node { leading, trailing, value }));
        }
        self.depth -= 1;
        let multiline = multiline && !members.is_empty();
        Ok(Value::Object { members, multiline, end: pending })
    }
}
