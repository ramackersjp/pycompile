use crate::ast::*;
use crate::error::{CompileError, Result};
use crate::lexer::{Lexer, Tok, Token};

/// Parse a single Python expression, used for f-string replacement fields.
pub fn parse_expression(src: &str, file: &str) -> Result<Expr> {
    let toks = Lexer::new(src, file).tokenize()?;
    let mut p = Parser::new(toks, file);
    p.skip_newlines();
    let e = p.parse_test()?;
    if !matches!(p.tok(), Tok::Newline | Tok::Eof) {
        return Err(p.err("unexpected token in f-string expression"));
    }
    Ok(e)
}

pub struct Parser<'a> {
    toks: Vec<Token>,
    pos: usize,
    file: &'a str,
}

impl<'a> Parser<'a> {
    pub fn new(toks: Vec<Token>, file: &'a str) -> Self {
        Parser { toks, pos: 0, file }
    }

    fn cur(&self) -> &Token {
        &self.toks[self.pos.min(self.toks.len() - 1)]
    }

    fn tok(&self) -> &Tok {
        &self.cur().tok
    }

    fn tok_at(&self, off: usize) -> &Tok {
        let i = (self.pos + off).min(self.toks.len() - 1);
        &self.toks[i].tok
    }

    fn line(&self) -> usize {
        self.cur().line
    }

    fn col(&self) -> usize {
        self.cur().col
    }

    fn err(&self, msg: impl Into<String>) -> CompileError {
        CompileError::new(self.file, self.line(), self.col(), msg)
    }

    fn advance(&mut self) -> Token {
        let t = self.toks[self.pos.min(self.toks.len() - 1)].clone();
        if self.pos < self.toks.len() - 1 {
            self.pos += 1;
        }
        t
    }

    fn at_op(&self, s: &str) -> bool {
        matches!(self.tok(), Tok::Op(o) if *o == s)
    }

    fn at_kw(&self, s: &str) -> bool {
        matches!(self.tok(), Tok::Keyword(k) if *k == s)
    }

    fn at_name(&self) -> bool {
        matches!(self.tok(), Tok::Name(_))
    }

    fn eat_op(&mut self, s: &str) -> bool {
        if self.at_op(s) {
            self.advance();
            true
        } else {
            false
        }
    }

    fn eat_kw(&mut self, s: &str) -> bool {
        if self.at_kw(s) {
            self.advance();
            true
        } else {
            false
        }
    }

    fn expect_op(&mut self, s: &str) -> Result<()> {
        if self.eat_op(s) {
            Ok(())
        } else {
            Err(self.err(format!("expected '{}', found {}", s, self.describe())))
        }
    }

    fn expect_kw(&mut self, s: &str) -> Result<()> {
        if self.eat_kw(s) {
            Ok(())
        } else {
            Err(self.err(format!("expected '{}', found {}", s, self.describe())))
        }
    }

    fn expect_name(&mut self) -> Result<String> {
        match self.tok().clone() {
            Tok::Name(n) => {
                self.advance();
                Ok(n)
            }
            _ => Err(self.err(format!("expected a name, found {}", self.describe()))),
        }
    }

    fn describe(&self) -> String {
        match self.tok() {
            Tok::Name(n) => format!("'{}'", n),
            Tok::Int(v) => format!("'{}'", v),
            Tok::Float(v) => format!("'{}'", v),
            Tok::Str(_) => "a string".to_string(),
            Tok::FStr(_) => "an f-string".to_string(),
            Tok::Keyword(k) => format!("'{}'", k),
            Tok::Op(o) => format!("'{}'", o),
            Tok::Newline => "end of line".to_string(),
            Tok::Indent => "an indent".to_string(),
            Tok::Dedent => "a dedent".to_string(),
            Tok::Eof => "end of file".to_string(),
        }
    }

    fn skip_newlines(&mut self) {
        while matches!(self.tok(), Tok::Newline) {
            self.advance();
        }
    }

    pub fn parse_module(&mut self) -> Result<Module> {
        let mut body = Vec::new();
        self.skip_newlines();
        while !matches!(self.tok(), Tok::Eof) {
            body.push(self.parse_statement()?);
            self.skip_newlines();
        }
        Ok(Module { body })
    }

    fn parse_statement(&mut self) -> Result<Stmt> {
        if matches!(self.tok(), Tok::Indent) {
            return Err(self.err("unexpected indent"));
        }
        if self.at_kw("if") {
            return self.parse_if();
        }
        if self.at_kw("while") {
            return self.parse_while();
        }
        if self.at_kw("for") {
            return self.parse_for();
        }
        if self.at_kw("def") {
            return self.parse_def();
        }
        if self.at_kw("with") || self.at_kw("try") || self.at_kw("class") || self.at_kw("async") {
            return Err(self.err(format!(
                "'{}' statements are not supported by this compiler yet",
                if let Tok::Keyword(k) = self.tok() {
                    *k
                } else {
                    ""
                }
            )));
        }
        if self.at_op("@") {
            return Err(self.err("decorators are not supported by this compiler yet"));
        }
        self.parse_simple_line()
    }

    fn parse_simple_line(&mut self) -> Result<Stmt> {
        let s = self.parse_simple_statement()?;
        if self.eat_op(";") && !matches!(self.tok(), Tok::Newline | Tok::Eof) {
            return Err(
                self.err("multiple statements on one line separated by ';' are not supported")
            );
        }
        if !matches!(self.tok(), Tok::Newline | Tok::Eof) {
            return Err(self.err(format!("unexpected {} after statement", self.describe())));
        }
        Ok(s)
    }

    fn parse_simple_statement(&mut self) -> Result<Stmt> {
        if self.at_kw("return") {
            self.advance();
            if matches!(self.tok(), Tok::Newline | Tok::Eof | Tok::Dedent) {
                return Ok(Stmt::Return(None));
            }
            let e = self.parse_testlist()?;
            return Ok(Stmt::Return(Some(e)));
        }
        if self.at_kw("pass") {
            self.advance();
            return Ok(Stmt::Pass);
        }
        if self.at_kw("break") {
            self.advance();
            return Ok(Stmt::Break);
        }
        if self.at_kw("continue") {
            self.advance();
            return Ok(Stmt::Continue);
        }
        if self.at_kw("import") {
            return self.parse_import();
        }
        if self.at_kw("from") {
            return self.parse_from_import();
        }
        if self.at_kw("global") {
            self.advance();
            let mut names = vec![self.expect_name()?];
            while self.eat_op(",") {
                names.push(self.expect_name()?);
            }
            return Ok(Stmt::Global(names));
        }
        if self.at_kw("assert") {
            self.advance();
            let e = self.parse_test()?;
            let msg = if self.eat_op(",") {
                Some(self.parse_test()?)
            } else {
                None
            };
            return Ok(Stmt::Assert(e, msg));
        }
        if self.at_kw("del") {
            self.advance();
            let mut targets = vec![self.parse_test()?];
            while self.eat_op(",") {
                targets.push(self.parse_test()?);
            }
            return Ok(Stmt::Del(targets));
        }
        if self.at_kw("raise") || self.at_kw("yield") {
            return Err(self.err("this statement is not supported by this compiler yet"));
        }
        self.parse_expr_or_assign()
    }

    fn parse_import(&mut self) -> Result<Stmt> {
        self.expect_kw("import")?;
        let module = self.expect_name()?;
        let alias = if self.eat_kw("as") {
            Some(self.expect_name()?)
        } else {
            None
        };
        Ok(Stmt::Import { module, alias })
    }

    fn parse_from_import(&mut self) -> Result<Stmt> {
        self.expect_kw("from")?;
        let module = self.expect_name()?;
        self.expect_kw("import")?;
        let mut names = Vec::new();
        loop {
            let n = self.expect_name()?;
            let alias = if self.eat_kw("as") {
                Some(self.expect_name()?)
            } else {
                None
            };
            names.push((n, alias));
            if !self.eat_op(",") {
                break;
            }
        }
        Ok(Stmt::FromImport { module, names })
    }

    fn parse_expr_or_assign(&mut self) -> Result<Stmt> {
        let first = self.parse_testlist()?;
        if self.eat_op(":") {
            let _ann = self.parse_test()?;
            let value = if self.eat_op("=") {
                Some(self.parse_testlist()?)
            } else {
                None
            };
            return Ok(match value {
                Some(value) => Stmt::Assign {
                    targets: vec![first],
                    value,
                },
                None => Stmt::Pass,
            });
        }
        if let Some(op) = self.current_augop() {
            self.advance();
            let value = self.parse_testlist()?;
            return Ok(Stmt::AugAssign {
                target: first,
                op,
                value,
            });
        }
        if self.at_op("=") {
            let mut targets = vec![first];
            loop {
                self.expect_op("=")?;
                let rhs = self.parse_testlist()?;
                if self.at_op("=") {
                    targets.push(rhs);
                } else {
                    return Ok(Stmt::Assign {
                        targets,
                        value: rhs,
                    });
                }
            }
        }
        Ok(Stmt::Expr(first))
    }

    fn current_augop(&self) -> Option<BinOp> {
        let op = match self.tok() {
            Tok::Op(o) => *o,
            _ => return None,
        };
        Some(match op {
            "+=" => BinOp::Add,
            "-=" => BinOp::Sub,
            "*=" => BinOp::Mul,
            "/=" => BinOp::Div,
            "//=" => BinOp::FloorDiv,
            "%=" => BinOp::Mod,
            "**=" => BinOp::Pow,
            "<<=" => BinOp::LShift,
            ">>=" => BinOp::RShift,
            "&=" => BinOp::BitAnd,
            "|=" => BinOp::BitOr,
            "^=" => BinOp::BitXor,
            _ => return None,
        })
    }

    fn parse_if(&mut self) -> Result<Stmt> {
        let mut branches = Vec::new();
        let mut orelse = Vec::new();
        self.expect_kw("if")?;
        let cond = self.parse_test()?;
        self.expect_op(":")?;
        let body = self.parse_suite()?;
        branches.push((cond, body));
        loop {
            if self.at_kw("elif") {
                self.advance();
                let c = self.parse_test()?;
                self.expect_op(":")?;
                let b = self.parse_suite()?;
                branches.push((c, b));
            } else if self.at_kw("else") {
                self.advance();
                self.expect_op(":")?;
                orelse = self.parse_suite()?;
                break;
            } else {
                break;
            }
        }
        Ok(Stmt::If { branches, orelse })
    }

    fn parse_while(&mut self) -> Result<Stmt> {
        self.expect_kw("while")?;
        let cond = self.parse_test()?;
        self.expect_op(":")?;
        let body = self.parse_suite()?;
        let orelse = self.parse_else_clause()?;
        Ok(Stmt::While { cond, body, orelse })
    }

    fn parse_for(&mut self) -> Result<Stmt> {
        self.expect_kw("for")?;
        let target = self.parse_target_list()?;
        self.expect_kw("in")?;
        let iter = self.parse_testlist()?;
        self.expect_op(":")?;
        let body = self.parse_suite()?;
        let orelse = self.parse_else_clause()?;
        Ok(Stmt::For {
            target,
            iter,
            body,
            orelse,
        })
    }

    fn parse_else_clause(&mut self) -> Result<Vec<Stmt>> {
        if self.at_kw("else") {
            self.advance();
            self.expect_op(":")?;
            return self.parse_suite();
        }
        Ok(Vec::new())
    }

    fn parse_def(&mut self) -> Result<Stmt> {
        self.expect_kw("def")?;
        let name = self.expect_name()?;
        self.expect_op("(")?;
        let mut params = Vec::new();
        let mut vararg = None;
        let mut kwarg = None;
        while !self.at_op(")") {
            if self.at_op("**") {
                self.advance();
                kwarg = Some(self.expect_name()?);
                break;
            }
            if self.at_op("*") {
                self.advance();
                if self.at_op(",") {
                    return Err(self.err("keyword-only parameters are not supported"));
                }
                vararg = Some(self.expect_name()?);
                if !self.eat_op(",") {
                    break;
                }
                continue;
            }
            if self.at_op("/") {
                return Err(self.err("positional-only markers are not supported"));
            }
            let pname = self.expect_name()?;
            if self.eat_op(":") {
                let _ = self.parse_test()?;
            }
            let default = if self.eat_op("=") {
                Some(self.parse_test()?)
            } else {
                None
            };
            params.push(Param {
                name: pname,
                default,
            });
            if !self.eat_op(",") {
                break;
            }
        }
        self.expect_op(")")?;
        if self.eat_op("->") {
            let _ = self.parse_test()?;
        }
        self.expect_op(":")?;
        let body = self.parse_suite()?;
        Ok(Stmt::FunctionDef {
            name,
            params,
            vararg,
            kwarg,
            body,
        })
    }

    fn parse_suite(&mut self) -> Result<Vec<Stmt>> {
        if matches!(self.tok(), Tok::Newline) {
            self.advance();
            match self.tok() {
                Tok::Indent => {
                    self.advance();
                }
                _ => {
                    // Empty block (e.g. function with only comments).
                    return Ok(Vec::new());
                }
            }
            let mut stmts = Vec::new();
            loop {
                match self.tok() {
                    Tok::Dedent => {
                        self.advance();
                        break;
                    }
                    Tok::Eof => break,
                    Tok::Newline => {
                        self.advance();
                    }
                    _ => stmts.push(self.parse_statement()?),
                }
            }
            Ok(stmts)
        } else {
            // Single-line suite: if x: a = 1
            let s = self.parse_simple_statement()?;
            if !matches!(self.tok(), Tok::Newline | Tok::Eof) {
                return Err(self.err("unexpected token in single-line suite"));
            }
            Ok(vec![s])
        }
    }

    // ---- Expressions ----

    fn parse_target_list(&mut self) -> Result<Expr> {
        let first = self.parse_bit_or()?;
        if !self.at_op(",") {
            return Ok(first);
        }
        let mut items = vec![first];
        while self.eat_op(",") {
            if self.at_kw("in") {
                break;
            }
            items.push(self.parse_bit_or()?);
        }
        Ok(Expr::Tuple(items))
    }

    /// comma separated expressions; yields a tuple when there is more than one.
    fn parse_testlist(&mut self) -> Result<Expr> {
        let first = self.parse_test()?;
        if !self.at_op(",") {
            return Ok(first);
        }
        let mut items = vec![first];
        while self.eat_op(",") {
            if self.stops_testlist() {
                break;
            }
            items.push(self.parse_test()?);
        }
        Ok(Expr::Tuple(items))
    }

    fn stops_testlist(&self) -> bool {
        matches!(
            self.tok(),
            Tok::Newline
                | Tok::Eof
                | Tok::Op(")")
                | Tok::Op("]")
                | Tok::Op("}")
                | Tok::Op(":")
                | Tok::Op("=")
        )
    }

    fn parse_test(&mut self) -> Result<Expr> {
        let body = self.parse_or()?;
        if self.at_kw("if") {
            self.advance();
            let cond = self.parse_or()?;
            self.expect_kw("else")?;
            let orelse = self.parse_test()?;
            return Ok(Expr::IfExp {
                cond: Box::new(cond),
                body: Box::new(body),
                orelse: Box::new(orelse),
            });
        }
        Ok(body)
    }

    fn parse_or(&mut self) -> Result<Expr> {
        let mut parts = vec![self.parse_and()?];
        while self.at_kw("or") {
            self.advance();
            parts.push(self.parse_and()?);
        }
        if parts.len() == 1 {
            Ok(parts.pop().unwrap())
        } else {
            Ok(Expr::BoolOp(BoolOp::Or, parts))
        }
    }

    fn parse_and(&mut self) -> Result<Expr> {
        let mut parts = vec![self.parse_not()?];
        while self.at_kw("and") {
            self.advance();
            parts.push(self.parse_not()?);
        }
        if parts.len() == 1 {
            Ok(parts.pop().unwrap())
        } else {
            Ok(Expr::BoolOp(BoolOp::And, parts))
        }
    }

    fn parse_not(&mut self) -> Result<Expr> {
        if self.at_kw("not") {
            self.advance();
            let e = self.parse_not()?;
            return Ok(Expr::UnaryOp(UnOp::Not, Box::new(e)));
        }
        self.parse_comparison()
    }

    fn parse_comparison(&mut self) -> Result<Expr> {
        let left = self.parse_bit_or()?;
        let mut ops = Vec::new();
        loop {
            let op = if self.at_op("==") {
                CmpOp::Eq
            } else if self.at_op("!=") {
                CmpOp::Ne
            } else if self.at_op("<=") {
                CmpOp::Le
            } else if self.at_op(">=") {
                CmpOp::Ge
            } else if self.at_op("<") {
                CmpOp::Lt
            } else if self.at_op(">") {
                CmpOp::Gt
            } else if self.at_kw("in") {
                CmpOp::In
            } else if self.at_kw("not") && matches!(self.tok_at(1), Tok::Keyword("in")) {
                CmpOp::NotIn
            } else if self.at_kw("is") && matches!(self.tok_at(1), Tok::Keyword("not")) {
                CmpOp::IsNot
            } else if self.at_kw("is") {
                CmpOp::Is
            } else {
                break;
            };
            match op {
                CmpOp::NotIn | CmpOp::IsNot => {
                    self.advance();
                    self.advance();
                }
                _ => {
                    self.advance();
                }
            }
            let right = self.parse_bit_or()?;
            ops.push((op, right));
        }
        if ops.is_empty() {
            Ok(left)
        } else {
            Ok(Expr::Compare(Box::new(left), ops))
        }
    }

    fn parse_bit_or(&mut self) -> Result<Expr> {
        let mut left = self.parse_bit_xor()?;
        while self.at_op("|") {
            self.advance();
            let right = self.parse_bit_xor()?;
            left = Expr::BinOp(Box::new(left), BinOp::BitOr, Box::new(right));
        }
        Ok(left)
    }

    fn parse_bit_xor(&mut self) -> Result<Expr> {
        let mut left = self.parse_bit_and()?;
        while self.at_op("^") {
            self.advance();
            let right = self.parse_bit_and()?;
            left = Expr::BinOp(Box::new(left), BinOp::BitXor, Box::new(right));
        }
        Ok(left)
    }

    fn parse_bit_and(&mut self) -> Result<Expr> {
        let mut left = self.parse_shift()?;
        while self.at_op("&") {
            self.advance();
            let right = self.parse_shift()?;
            left = Expr::BinOp(Box::new(left), BinOp::BitAnd, Box::new(right));
        }
        Ok(left)
    }

    fn parse_shift(&mut self) -> Result<Expr> {
        let mut left = self.parse_arith()?;
        loop {
            if self.at_op("<<") {
                self.advance();
                let right = self.parse_arith()?;
                left = Expr::BinOp(Box::new(left), BinOp::LShift, Box::new(right));
            } else if self.at_op(">>") {
                self.advance();
                let right = self.parse_arith()?;
                left = Expr::BinOp(Box::new(left), BinOp::RShift, Box::new(right));
            } else {
                break;
            }
        }
        Ok(left)
    }

    fn parse_arith(&mut self) -> Result<Expr> {
        let mut left = self.parse_term()?;
        loop {
            let op = if self.at_op("+") {
                BinOp::Add
            } else if self.at_op("-") {
                BinOp::Sub
            } else {
                break;
            };
            self.advance();
            let right = self.parse_term()?;
            left = Expr::BinOp(Box::new(left), op, Box::new(right));
        }
        Ok(left)
    }

    fn parse_term(&mut self) -> Result<Expr> {
        let mut left = self.parse_factor()?;
        loop {
            let op = if self.at_op("*") {
                BinOp::Mul
            } else if self.at_op("/") {
                BinOp::Div
            } else if self.at_op("//") {
                BinOp::FloorDiv
            } else if self.at_op("%") {
                BinOp::Mod
            } else {
                break;
            };
            self.advance();
            let right = self.parse_factor()?;
            left = Expr::BinOp(Box::new(left), op, Box::new(right));
        }
        Ok(left)
    }

    fn parse_factor(&mut self) -> Result<Expr> {
        if self.at_op("+") {
            self.advance();
            return Ok(Expr::UnaryOp(UnOp::Pos, Box::new(self.parse_factor()?)));
        }
        if self.at_op("-") {
            self.advance();
            return Ok(Expr::UnaryOp(UnOp::Neg, Box::new(self.parse_factor()?)));
        }
        if self.at_op("~") {
            self.advance();
            return Ok(Expr::UnaryOp(UnOp::Invert, Box::new(self.parse_factor()?)));
        }
        self.parse_power()
    }

    fn parse_power(&mut self) -> Result<Expr> {
        let base = self.parse_unary_postfix()?;
        if self.at_op("**") {
            self.advance();
            let exp = self.parse_factor()?;
            return Ok(Expr::BinOp(Box::new(base), BinOp::Pow, Box::new(exp)));
        }
        Ok(base)
    }

    fn parse_unary_postfix(&mut self) -> Result<Expr> {
        let mut e = self.parse_atom()?;
        loop {
            if self.at_op("(") {
                e = self.parse_call(e)?;
            } else if self.at_op("[") {
                self.advance();
                e = self.parse_subscript(e)?;
            } else if self.at_op(".") {
                self.advance();
                let name = self.expect_name()?;
                e = Expr::Attribute(Box::new(e), name);
            } else {
                break;
            }
        }
        Ok(e)
    }

    fn parse_call(&mut self, func: Expr) -> Result<Expr> {
        self.expect_op("(")?;
        let mut args = Vec::new();
        let mut kwargs = Vec::new();
        let mut star_args = Vec::new();
        let mut dstar = None;
        while !self.at_op(")") {
            if self.at_op("*") {
                self.advance();
                star_args.push(self.parse_test()?);
            } else if self.at_op("**") {
                self.advance();
                dstar = Some(Box::new(self.parse_test()?));
            } else if self.at_name() && matches!(self.tok_at(1), Tok::Op("=")) {
                let name = self.expect_name()?;
                self.expect_op("=")?;
                kwargs.push((name, self.parse_test()?));
            } else {
                args.push(self.parse_test()?);
            }
            if !self.eat_op(",") {
                break;
            }
        }
        self.expect_op(")")?;
        Ok(Expr::Call {
            func: Box::new(func),
            args,
            kwargs,
            star_args,
            dstar,
        })
    }

    fn parse_subscript(&mut self, base: Expr) -> Result<Expr> {
        // Parse a possibly sliced index.
        let lower = if self.at_op(":") {
            None
        } else {
            Some(self.parse_test()?)
        };
        if self.at_op(":") {
            self.advance();
            let upper = if self.at_op(":") || self.at_op("]") {
                None
            } else {
                Some(Box::new(self.parse_test()?))
            };
            let step = if self.eat_op(":") {
                if self.at_op("]") {
                    None
                } else {
                    Some(Box::new(self.parse_test()?))
                }
            } else {
                None
            };
            self.expect_op("]")?;
            return Ok(Expr::Subscript(
                Box::new(base),
                Box::new(Expr::Slice {
                    lower: lower.map(Box::new),
                    upper,
                    step,
                }),
            ));
        }
        self.expect_op("]")?;
        Ok(Expr::Subscript(Box::new(base), Box::new(lower.unwrap())))
    }

    fn parse_atom(&mut self) -> Result<Expr> {
        match self.tok().clone() {
            Tok::Int(v) => {
                self.advance();
                Ok(Expr::Int(v))
            }
            Tok::Float(v) => {
                self.advance();
                Ok(Expr::Float(v))
            }
            Tok::Str(s) => {
                self.advance();
                let mut all = s;
                while let Tok::Str(s2) = self.tok().clone() {
                    self.advance();
                    all.push_str(&s2);
                }
                Ok(Expr::Str(all))
            }
            Tok::FStr(_) => {
                let mut parts = match self.tok().clone() {
                    Tok::FStr(p) => p,
                    _ => unreachable!(),
                };
                self.advance();
                while let Tok::FStr(p2) = self.tok().clone() {
                    parts.extend(p2);
                    self.advance();
                }
                Ok(Expr::FStr(parts))
            }
            Tok::Name(n) => {
                self.advance();
                Ok(Expr::Name(n))
            }
            Tok::Keyword("True") => {
                self.advance();
                Ok(Expr::Bool(true))
            }
            Tok::Keyword("False") => {
                self.advance();
                Ok(Expr::Bool(false))
            }
            Tok::Keyword("None") => {
                self.advance();
                Ok(Expr::None)
            }
            Tok::Op("(") => self.parse_paren(),
            Tok::Op("[") => self.parse_bracket(),
            Tok::Op("{") => self.parse_brace(),
            _ => Err(self.err(format!("unexpected {}", self.describe()))),
        }
    }

    fn parse_paren(&mut self) -> Result<Expr> {
        self.expect_op("(")?;
        if self.at_op(")") {
            self.advance();
            return Ok(Expr::Tuple(Vec::new()));
        }
        let first = self.parse_test()?;
        if self.at_kw("for") {
            let (target, iter, cond) = self.parse_comp_clauses()?;
            self.expect_op(")")?;
            return Ok(Expr::ListComp {
                elt: Box::new(first),
                target: Box::new(target),
                iter: Box::new(iter),
                cond,
            });
        }
        if self.at_op(",") {
            let mut items = vec![first];
            while self.eat_op(",") {
                if self.at_op(")") {
                    break;
                }
                items.push(self.parse_test()?);
            }
            self.expect_op(")")?;
            return Ok(Expr::Tuple(items));
        }
        self.expect_op(")")?;
        Ok(first)
    }

    fn parse_bracket(&mut self) -> Result<Expr> {
        self.expect_op("[")?;
        if self.at_op("]") {
            self.advance();
            return Ok(Expr::List(Vec::new()));
        }
        let first = self.parse_test()?;
        if self.at_kw("for") {
            let (target, iter, cond) = self.parse_comp_clauses()?;
            self.expect_op("]")?;
            return Ok(Expr::ListComp {
                elt: Box::new(first),
                target: Box::new(target),
                iter: Box::new(iter),
                cond,
            });
        }
        let mut items = vec![first];
        while self.eat_op(",") {
            if self.at_op("]") {
                break;
            }
            items.push(self.parse_test()?);
        }
        self.expect_op("]")?;
        Ok(Expr::List(items))
    }

    fn parse_brace(&mut self) -> Result<Expr> {
        self.expect_op("{")?;
        if self.at_op("}") {
            self.advance();
            return Ok(Expr::Dict(Vec::new()));
        }
        if self.at_op("**") {
            return Err(self.err("dict unpacking in literals is not supported"));
        }
        let first = self.parse_test()?;
        if self.eat_op(":") {
            let v = self.parse_test()?;
            if self.at_kw("for") {
                let (target, iter, cond) = self.parse_comp_clauses()?;
                self.expect_op("}")?;
                return Ok(Expr::DictComp {
                    key: Box::new(first),
                    value: Box::new(v),
                    target: Box::new(target),
                    iter: Box::new(iter),
                    cond,
                });
            }
            let mut pairs = vec![(Some(first), v)];
            while self.eat_op(",") {
                if self.at_op("}") {
                    break;
                }
                let k = self.parse_test()?;
                self.expect_op(":")?;
                let val = self.parse_test()?;
                pairs.push((Some(k), val));
            }
            self.expect_op("}")?;
            return Ok(Expr::Dict(pairs));
        }
        if self.at_kw("for") {
            let (target, iter, cond) = self.parse_comp_clauses()?;
            self.expect_op("}")?;
            return Ok(Expr::SetComp {
                elt: Box::new(first),
                target: Box::new(target),
                iter: Box::new(iter),
                cond,
            });
        }
        // A set literal.
        let mut items = vec![first];
        while self.eat_op(",") {
            if self.at_op("}") {
                break;
            }
            items.push(self.parse_test()?);
        }
        self.expect_op("}")?;
        Ok(Expr::Set(items))
    }

    fn parse_comp_clauses(&mut self) -> Result<(Expr, Expr, Option<Box<Expr>>)> {
        self.expect_kw("for")?;
        let target = self.parse_target_list()?;
        self.expect_kw("in")?;
        let iter = self.parse_or()?;
        let cond = if self.at_kw("if") {
            self.advance();
            Some(Box::new(self.parse_or()?))
        } else {
            None
        };
        Ok((target, iter, cond))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::Lexer;

    fn parse(src: &str) -> Module {
        let toks = Lexer::new(src, "test").tokenize().unwrap();
        Parser::new(toks, "test").parse_module().unwrap()
    }

    #[test]
    fn assignment_and_call() {
        let m = parse("x = 1\nprint(x + 2)\n");
        assert_eq!(m.body.len(), 2);
    }

    #[test]
    fn function_and_if() {
        let m = parse("def f(a, b=2):\n    if a > b:\n        return a\n    return b\n");
        assert_eq!(m.body.len(), 1);
    }

    #[test]
    fn loops() {
        parse("for i in range(10):\n    print(i)\nelse:\n    pass\n");
        parse("while x < 3:\n    x += 1\n");
    }

    #[test]
    fn collections() {
        parse("d = {'a': 1, 'b': 2}\n");
        parse("l = [1, 2, 3]\n");
        parse("t = (1, 2)\n");
        parse("s = {1, 2, 3}\n");
        parse("c = [x * 2 for x in range(5) if x > 1]\n");
    }
}
