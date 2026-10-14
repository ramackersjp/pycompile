use crate::error::{CompileError, Result};

#[derive(Debug, Clone, PartialEq)]
pub enum FPart {
    Lit(String),
    /// `{expr}` or `{expr:spec}`
    Expr {
        src: String,
        spec: String,
        repr: bool,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum Tok {
    Name(String),
    Int(i64),
    Float(f64),
    Str(String),
    FStr(Vec<FPart>),
    Keyword(&'static str),
    Op(&'static str),
    Newline,
    Indent,
    Dedent,
    Eof,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    pub tok: Tok,
    pub line: usize,
    pub col: usize,
}

pub const KEYWORDS: &[&str] = &[
    "and", "or", "not", "if", "elif", "else", "while", "for", "in", "def", "return", "break",
    "continue", "pass", "True", "False", "None", "import", "from", "as", "class", "lambda",
    "global", "del", "with", "try", "except", "finally", "raise", "assert", "is", "nonlocal",
    "yield", "async", "await",
];

fn is_keyword(s: &str) -> Option<&'static str> {
    KEYWORDS.iter().copied().find(|k| *k == s)
}

pub struct Lexer<'a> {
    src: Vec<char>,
    file: &'a str,
    pos: usize,
    line: usize,
    line_start: usize,
    indents: Vec<usize>,
    depth: i32,
    out: Vec<Token>,
    produced: bool,
}

impl<'a> Lexer<'a> {
    pub fn new(src: &'a str, file: &'a str) -> Self {
        Lexer {
            src: src.chars().collect(),
            file,
            pos: 0,
            line: 1,
            line_start: 0,
            indents: vec![0],
            depth: 0,
            out: Vec::new(),
            produced: false,
        }
    }

    fn err(&self, msg: impl Into<String>) -> CompileError {
        CompileError::new(self.file, self.line, self.pos - self.line_start + 1, msg)
    }

    fn at_end(&self) -> bool {
        self.pos >= self.src.len()
    }

    fn peek(&self) -> char {
        if self.pos < self.src.len() {
            self.src[self.pos]
        } else {
            '\0'
        }
    }

    fn peek_at(&self, off: usize) -> char {
        if self.pos + off < self.src.len() {
            self.src[self.pos + off]
        } else {
            '\0'
        }
    }

    fn bump(&mut self) -> char {
        let c = self.peek();
        self.pos += 1;
        c
    }

    fn push(&mut self, tok: Tok, line: usize, col: usize) {
        self.out.push(Token { tok, line, col });
    }

    pub fn tokenize(mut self) -> Result<Vec<Token>> {
        loop {
            if !self.scan_line_start()? {
                break;
            }
            self.scan_tokens()?;
        }
        // Final newline if the last logical line had tokens.
        if self.produced {
            self.produced = false;
            self.push(Tok::Newline, self.line, 1);
        }
        while self.indents.len() > 1 {
            self.indents.pop();
            self.push(Tok::Dedent, self.line, 1);
        }
        self.push(Tok::Eof, self.line, 1);
        Ok(self.out)
    }

    fn scan_line_start(&mut self) -> Result<bool> {
        if self.at_end() {
            return Ok(false);
        }
        // Measure indentation.
        let mut indent = 0usize;
        loop {
            match self.peek() {
                ' ' => {
                    indent += 1;
                    self.pos += 1;
                }
                '\t' => {
                    indent = (indent / 8 + 1) * 8;
                    self.pos += 1;
                }
                _ => break,
            }
        }
        if self.at_end() {
            return Ok(false);
        }
        if self.peek() == '\n' {
            self.pos += 1;
            self.line += 1;
            self.line_start = self.pos;
            self.produced = false;
            return self.scan_line_start();
        }
        if self.peek() == '#' {
            while !self.at_end() && self.peek() != '\n' {
                self.pos += 1;
            }
            return self.scan_line_start();
        }
        if self.peek() == '\r' {
            self.pos += 1;
            return self.scan_line_start();
        }
        let top = *self.indents.last().unwrap();
        if indent > top {
            self.indents.push(indent);
            self.push(Tok::Indent, self.line, 1);
        } else if indent < top {
            while *self.indents.last().unwrap() > indent {
                self.indents.pop();
                self.push(Tok::Dedent, self.line, 1);
            }
            if *self.indents.last().unwrap() != indent {
                return Err(self.err("unindent does not match any outer indentation level"));
            }
        }
        self.produced = false;
        Ok(true)
    }

    fn scan_tokens(&mut self) -> Result<()> {
        loop {
            if self.at_end() {
                return Ok(());
            }
            let c = self.peek();
            let line = self.line;
            let col = self.pos - self.line_start + 1;
            if c == '\n' {
                self.pos += 1;
                self.line += 1;
                self.line_start = self.pos;
                if self.depth == 0 {
                    if self.produced {
                        self.produced = false;
                        self.push(Tok::Newline, line, col);
                    }
                    return Ok(());
                } else {
                    continue;
                }
            }
            if c == ' ' || c == '\t' || c == '\r' {
                self.pos += 1;
                continue;
            }
            if c == '\\' && self.peek_at(1) == '\n' {
                self.pos += 2;
                self.line += 1;
                self.line_start = self.pos;
                continue;
            }
            if c == '\\' && self.peek_at(1) == '\r' && self.peek_at(2) == '\n' {
                self.pos += 3;
                self.line += 1;
                self.line_start = self.pos;
                continue;
            }
            if c == '#' {
                while !self.at_end() && self.peek() != '\n' {
                    self.pos += 1;
                }
                continue;
            }
            if c.is_ascii_digit() || (c == '.' && self.peek_at(1).is_ascii_digit()) {
                self.read_number()?;
                self.produced = true;
                continue;
            }
            if c == '_' || c.is_alphabetic() {
                self.read_name_or_string()?;
                self.produced = true;
                continue;
            }
            if c == '"' || c == '\'' {
                self.read_string("", line, col)?;
                self.produced = true;
                continue;
            }
            self.read_operator()?;
            self.produced = true;
        }
    }

    fn read_number(&mut self) -> Result<()> {
        let line = self.line;
        let col = self.pos - self.line_start + 1;
        let start = self.pos;
        let mut text = String::new();
        if self.peek() == '0' {
            let n = self.peek_at(1);
            if n == 'x' || n == 'X' || n == 'o' || n == 'O' || n == 'b' || n == 'B' {
                text.push(self.bump());
                text.push(self.bump());
                while self.peek().is_ascii_alphanumeric() || self.peek() == '_' {
                    text.push(self.bump());
                }
                let clean: String = text.chars().filter(|c| *c != '_').collect();
                let radix = match clean.as_bytes()[1] {
                    b'x' | b'X' => 16,
                    b'o' | b'O' => 8,
                    _ => 2,
                };
                let v = i64::from_str_radix(&clean[2..], radix)
                    .map_err(|_| self.err("invalid integer literal"))?;
                self.push(Tok::Int(v), line, col);
                return Ok(());
            }
        }
        let mut is_float = false;
        while self.peek().is_ascii_digit() || self.peek() == '_' {
            text.push(self.bump());
        }
        if self.peek() == '.' && self.peek_at(1) != '.' {
            is_float = true;
            text.push(self.bump());
            while self.peek().is_ascii_digit() || self.peek() == '_' {
                text.push(self.bump());
            }
        }
        if self.peek() == 'e' || self.peek() == 'E' {
            let save = self.pos;
            let mut exp = String::from(self.bump());
            if self.peek() == '+' || self.peek() == '-' {
                exp.push(self.bump());
            }
            if self.peek().is_ascii_digit() {
                is_float = true;
                while self.peek().is_ascii_digit() || self.peek() == '_' {
                    exp.push(self.bump());
                }
                text.push_str(&exp);
            } else {
                self.pos = save;
            }
        }
        let clean: String = text.chars().filter(|c| *c != '_').collect();
        if is_float {
            let v: f64 = clean
                .parse()
                .map_err(|_| self.err("invalid float literal"))?;
            self.push(Tok::Float(v), line, col);
        } else {
            let v: i64 = clean.parse().map_err(|_| {
                CompileError::new(self.file, line, col, "integer literal too large")
            })?;
            self.push(Tok::Int(v), line, col);
        }
        let _ = start;
        Ok(())
    }

    fn read_name_or_string(&mut self) -> Result<()> {
        let line = self.line;
        let col = self.pos - self.line_start + 1;
        let start = self.pos;
        while self.peek() == '_' || self.peek().is_alphanumeric() {
            self.pos += 1;
        }
        let word: String = self.src[start..self.pos].iter().collect();
        // String prefix?
        let lower = word.to_ascii_lowercase();
        let is_prefix = word.len() <= 2
            && lower.chars().all(|c| "rbuf".contains(c))
            && matches!(self.peek(), '"' | '\'');
        if is_prefix {
            return self.read_string(&lower, line, col);
        }
        if let Some(kw) = is_keyword(&word) {
            self.push(Tok::Keyword(kw), line, col);
        } else {
            self.push(Tok::Name(word), line, col);
        }
        Ok(())
    }

    fn read_string(&mut self, prefix: &str, line: usize, col: usize) -> Result<()> {
        let raw = prefix.contains('r');
        let is_f = prefix.contains('f');
        let quote = self.bump();
        let triple = self.peek() == quote && self.peek_at(1) == quote;
        if triple {
            self.pos += 2;
        }
        let mut parts: Vec<FPart> = Vec::new();
        let mut cur = String::new();
        loop {
            if self.at_end() {
                return Err(CompileError::new(
                    self.file,
                    line,
                    col,
                    "unterminated string literal",
                ));
            }
            let c = self.peek();
            if c == '\n' {
                if !triple {
                    return Err(CompileError::new(
                        self.file,
                        line,
                        col,
                        "unterminated string literal",
                    ));
                }
                self.pos += 1;
                self.line += 1;
                self.line_start = self.pos;
                cur.push('\n');
                continue;
            }
            if c == quote {
                if triple {
                    if self.peek_at(1) == quote && self.peek_at(2) == quote {
                        self.pos += 3;
                        break;
                    }
                    cur.push(self.bump());
                    continue;
                } else {
                    self.pos += 1;
                    break;
                }
            }
            if c == '\\' && !raw {
                self.pos += 1;
                let e = self.bump();
                match e {
                    'n' => cur.push('\n'),
                    't' => cur.push('\t'),
                    'r' => cur.push('\r'),
                    '0' => cur.push('\0'),
                    '\\' => cur.push('\\'),
                    '\'' => cur.push('\''),
                    '"' => cur.push('"'),
                    'a' => cur.push('\u{7}'),
                    'b' => cur.push('\u{8}'),
                    'f' => cur.push('\u{c}'),
                    'v' => cur.push('\u{b}'),
                    'x' => {
                        let h: String = (0..2).map(|_| self.bump()).collect();
                        let v = u8::from_str_radix(&h, 16).unwrap_or(0);
                        cur.push(v as char);
                    }
                    'u' => {
                        let h: String = (0..4).map(|_| self.bump()).collect();
                        let v = u32::from_str_radix(&h, 16).unwrap_or(0);
                        if let Some(ch) = char::from_u32(v) {
                            cur.push(ch);
                        }
                    }
                    'U' => {
                        let h: String = (0..8).map(|_| self.bump()).collect();
                        let v = u32::from_str_radix(&h, 16).unwrap_or(0);
                        if let Some(ch) = char::from_u32(v) {
                            cur.push(ch);
                        }
                    }
                    '\n' => {
                        self.line += 1;
                        self.line_start = self.pos;
                    }
                    other => {
                        cur.push('\\');
                        cur.push(other);
                    }
                }
                continue;
            }
            if is_f && c == '{' {
                if self.peek_at(1) == '{' {
                    self.pos += 2;
                    cur.push('{');
                    continue;
                }
                if !cur.is_empty() {
                    parts.push(FPart::Lit(std::mem::take(&mut cur)));
                }
                self.pos += 1;
                let (expr, spec, repr) = self.read_fstring_field(line, col)?;
                parts.push(FPart::Expr {
                    src: expr,
                    spec,
                    repr,
                });
                continue;
            }
            if is_f && c == '}' {
                if self.peek_at(1) == '}' {
                    self.pos += 2;
                    cur.push('}');
                    continue;
                }
                return Err(CompileError::new(
                    self.file,
                    self.line,
                    self.pos - self.line_start + 1,
                    "single '}' is not allowed in f-string",
                ));
            }
            cur.push(self.bump());
        }
        if is_f {
            if !cur.is_empty() || parts.is_empty() {
                parts.push(FPart::Lit(cur));
            }
            self.push(Tok::FStr(parts), line, col);
        } else {
            self.push(Tok::Str(cur), line, col);
        }
        Ok(())
    }

    /// Read the interior of an f-string replacement field, up to the matching `}`.
    fn read_fstring_field(&mut self, line: usize, col: usize) -> Result<(String, String, bool)> {
        let mut expr = String::new();
        let mut depth = 0i32;
        let mut repr = false;
        loop {
            if self.at_end() {
                return Err(CompileError::new(
                    self.file,
                    line,
                    col,
                    "unterminated f-string",
                ));
            }
            let c = self.peek();
            match c {
                '(' | '[' | '{' => {
                    depth += 1;
                    expr.push(self.bump());
                }
                ')' | ']' | '}' => {
                    if c == '}' && depth == 0 {
                        self.pos += 1;
                        return Ok((expr, String::new(), repr));
                    }
                    depth -= 1;
                    expr.push(self.bump());
                }
                ':' if depth == 0 => {
                    self.pos += 1;
                    let spec = self.read_spec(line, col)?;
                    return Ok((expr, spec, repr));
                }
                '!' if depth == 0 => {
                    self.pos += 1;
                    let conv = self.bump();
                    repr = conv == 'r' || conv == 'a';
                    // after conversion an optional spec may follow
                    if self.peek() == ':' {
                        self.pos += 1;
                        let spec = self.read_spec(line, col)?;
                        return Ok((expr, spec, repr));
                    }
                }
                '\'' | '"' => {
                    let q = self.bump();
                    expr.push(q);
                    while !self.at_end() && self.peek() != q {
                        if self.peek() == '\\' {
                            expr.push(self.bump());
                        }
                        if !self.at_end() {
                            expr.push(self.bump());
                        }
                    }
                    if !self.at_end() {
                        expr.push(self.bump());
                    }
                }
                '\n' => {
                    return Err(CompileError::new(
                        self.file,
                        line,
                        col,
                        "unterminated f-string",
                    ))
                }
                _ => expr.push(self.bump()),
            }
        }
    }

    fn read_spec(&mut self, line: usize, col: usize) -> Result<String> {
        let mut spec = String::new();
        let mut depth = 0i32;
        loop {
            if self.at_end() {
                return Err(CompileError::new(
                    self.file,
                    line,
                    col,
                    "unterminated f-string spec",
                ));
            }
            let c = self.peek();
            if c == '{' {
                depth += 1;
                spec.push(self.bump());
            } else if c == '}' {
                if depth == 0 {
                    self.pos += 1;
                    return Ok(spec);
                }
                depth -= 1;
                spec.push(self.bump());
            } else {
                spec.push(self.bump());
            }
        }
    }

    fn read_operator(&mut self) -> Result<()> {
        let line = self.line;
        let col = self.pos - self.line_start + 1;
        const THREE: &[&str] = &["**=", "//=", ">>=", "<<=", "..."];
        const TWO: &[&str] = &[
            "**", "//", ">>", "<<", ">=", "<=", "==", "!=", "->", ":=", "+=", "-=", "*=", "/=",
            "%=", "&=", "|=", "^=",
        ];
        let rest: String = self.src[self.pos..(self.pos + 3).min(self.src.len())]
            .iter()
            .collect();
        for op in THREE {
            if rest.starts_with(op) {
                if *op == "..." {
                    self.pos += 3;
                    self.push(Tok::Op("..."), line, col);
                    return Ok(());
                }
                self.pos += 3;
                self.push(Tok::Op(op), line, col);
                return Ok(());
            }
        }
        for op in TWO {
            if rest.starts_with(op) {
                self.pos += 2;
                self.push(Tok::Op(op), line, col);
                return Ok(());
            }
        }
        let c = self.bump();
        let s = match c {
            '(' | '[' | '{' => {
                self.depth += 1;
                match c {
                    '(' => "(",
                    '[' => "[",
                    _ => "{",
                }
            }
            ')' | ']' | '}' => {
                self.depth -= 1;
                match c {
                    ')' => ")",
                    ']' => "]",
                    _ => "}",
                }
            }
            '+' => "+",
            '-' => "-",
            '*' => "*",
            '/' => "/",
            '%' => "%",
            '=' => "=",
            '<' => "<",
            '>' => ">",
            ',' => ",",
            ':' => ":",
            '.' => ".",
            ';' => ";",
            '@' => "@",
            '&' => "&",
            '|' => "|",
            '^' => "^",
            '~' => "~",
            other => {
                return Err(self.err(format!("unexpected character '{}'", other)));
            }
        };
        self.push(Tok::Op(s), line, col);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn toks(src: &str) -> Vec<Tok> {
        Lexer::new(src, "test")
            .tokenize()
            .unwrap()
            .into_iter()
            .map(|t| t.tok)
            .collect()
    }

    #[test]
    fn simple_assignment() {
        let t = toks("x = 1 + 2\n");
        assert_eq!(
            t,
            vec![
                Tok::Name("x".into()),
                Tok::Op("="),
                Tok::Int(1),
                Tok::Op("+"),
                Tok::Int(2),
                Tok::Newline,
                Tok::Eof
            ]
        );
    }

    #[test]
    fn indentation() {
        let t = toks("if x:\n    y = 1\nz = 2\n");
        assert!(t.contains(&Tok::Indent));
        assert!(t.contains(&Tok::Dedent));
    }

    #[test]
    fn brackets_suppress_newline() {
        let t = toks("x = (\n  1,\n  2\n)\n");
        let newlines = t.iter().filter(|x| **x == Tok::Newline).count();
        assert_eq!(newlines, 1);
    }

    #[test]
    fn strings_and_fstrings() {
        let t = toks("s = f\"a{1 + 2:>{w}}b\"\n");
        match &t[2] {
            Tok::FStr(parts) => {
                assert_eq!(parts.len(), 3);
                assert_eq!(parts[0], FPart::Lit("a".into()));
            }
            other => panic!("expected fstring, got {:?}", other),
        }
    }
}
