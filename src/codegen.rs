use crate::ast::*;
use crate::error::{CompileError, Result};
use crate::lexer::FPart;
use crate::parser::parse_expression;
use std::collections::{HashMap, HashSet};

const BUILTINS: &[&str] = &[
    "print",
    "len",
    "range",
    "str",
    "repr",
    "int",
    "float",
    "bool",
    "abs",
    "min",
    "max",
    "sum",
    "sorted",
    "reversed",
    "enumerate",
    "zip",
    "any",
    "all",
    "list",
    "tuple",
    "set",
    "dict",
    "type",
    "isinstance",
    "round",
    "pow",
    "chr",
    "ord",
    "hex",
    "oct",
    "bin",
    "divmod",
    "input",
];

const FIRSTCLASS: &[&str] = &[
    "len", "abs", "int", "float", "str", "repr", "bool", "sum", "min", "max", "round",
];

const EXC_TYPES: &[&str] = &[
    "BaseException",
    "Exception",
    "ArithmeticError",
    "ZeroDivisionError",
    "OverflowError",
    "FloatingPointError",
    "LookupError",
    "IndexError",
    "KeyError",
    "AssertionError",
    "AttributeError",
    "NameError",
    "NotImplementedError",
    "RuntimeError",
    "StopIteration",
    "TypeError",
    "ValueError",
    "ImportError",
    "OSError",
    "EOFError",
    "UnicodeError",
    "KeyboardInterrupt",
    "SystemExit",
    "GeneratorExit",
];

const MATH_FUNCS: &[&str] = &[
    "sqrt",
    "sin",
    "cos",
    "tan",
    "asin",
    "acos",
    "atan",
    "atan2",
    "exp",
    "log",
    "log2",
    "log10",
    "pow",
    "floor",
    "ceil",
    "trunc",
    "fabs",
    "fmod",
    "hypot",
    "radians",
    "degrees",
    "isqrt",
    "gcd",
    "factorial",
    "isnan",
    "isinf",
];

const MATH_CONSTS: &[&str] = &["pi", "e", "tau", "inf", "nan"];

#[derive(Clone)]
enum FuncBody {
    Stmts(Vec<Stmt>),
    Expr(Expr),
    Class(Vec<Stmt>),
}

struct FuncDef {
    name: String,
    is_lambda: bool,
    bases: Vec<Expr>,
    cname: String,
    params: Vec<Param>,
    vararg: Option<String>,
    kwarg: Option<String>,
    body: FuncBody,
    globals: HashSet<String>,
    parent: Option<usize>,
    free: Vec<String>,
    cells: Vec<String>,
}

#[derive(Clone)]
enum Slot {
    Plain(String),
    Cell(String),
}

fn read_slot(slot: &Slot) -> String {
    match slot {
        Slot::Plain(v) => v.clone(),
        Slot::Cell(c) => format!("py_cell_get({})", c),
    }
}

enum CompKind<'a> {
    List(&'a Expr),
    Set(&'a Expr),
    Dict(&'a Expr, &'a Expr),
}

pub struct Codegen<'a> {
    file: &'a str,
    funcs: Vec<FuncDef>,
    func_index: HashMap<usize, usize>,
    module_globals: HashSet<String>,
    aliases: HashMap<String, String>,
    math_short: HashSet<String>,
    scopes: Vec<HashMap<String, Slot>>,
    tmp: usize,
    func_depth: usize,
    break_flags: Vec<Option<(Option<String>, usize)>>,
    handler_depth: usize,
    blocked_exit: usize,
}

impl<'a> Codegen<'a> {
    pub fn new(file: &'a str) -> Self {
        Codegen {
            file,
            funcs: Vec::new(),
            func_index: HashMap::new(),
            module_globals: HashSet::new(),
            aliases: HashMap::new(),
            math_short: HashSet::new(),
            scopes: Vec::new(),
            tmp: 0,
            func_depth: 0,
            break_flags: Vec::new(),
            handler_depth: 0,
            blocked_exit: 0,
        }
    }

    pub fn compile(mut self, module: &Module) -> Result<String> {
        self.prescan(&module.body)?;
        gather_globals(&module.body, &mut self.module_globals);
        let mut raw = Vec::new();
        collect_all(&module.body, None, &mut raw);
        for (i, rf) in raw.into_iter().enumerate() {
            let globals = match &rf.body {
                FuncBody::Stmts(stmts) => {
                    let mut g = HashSet::new();
                    gather_global_decls(stmts, &mut g);
                    g
                }
                _ => HashSet::new(),
            };
            if !rf.is_lambda && !rf.is_method && rf.parent.is_none() {
                self.module_globals.insert(rf.name.clone());
            }
            self.func_index.insert(rf.id, i);
            let label = if rf.is_lambda {
                "lambda".to_string()
            } else {
                rf.name.clone()
            };
            let prefix = if rf.is_class { "pyclass" } else { "pyfn" };
            let cname = format!("{}_{}_{}", prefix, sanitize(&label), i);
            self.funcs.push(FuncDef {
                name: rf.name,
                is_lambda: rf.is_lambda,
                bases: rf.bases,
                cname,
                params: rf.params,
                vararg: rf.vararg,
                kwarg: rf.kwarg,
                body: rf.body,
                globals,
                parent: rf.parent,
                free: Vec::new(),
                cells: Vec::new(),
            });
        }
        compute_frees(
            &mut self.funcs,
            &self.module_globals,
            &self.aliases,
            &self.math_short,
        );

        let mut out = String::new();
        out.push_str("/* Generated by pycompile. Do not edit. */\n");
        out.push_str("#include \"pyrt.h\"\n\n");

        for f in &self.funcs {
            out.push_str(&format!("static Pyo* {}(Pyo** args, int argc);\n", f.cname));
        }
        out.push('\n');

        let mut names: Vec<&String> = self.module_globals.iter().collect();
        names.sort();
        for n in &names {
            out.push_str(&format!("static Pyo* g_{};\n", sanitize(n)));
        }
        out.push('\n');

        // main
        out.push_str("int main(int argc, char** argv) {\n");
        out.push_str("#if defined(__GNUC__) && !defined(__TINYC__)\n");
        out.push_str("    py_gc_init(__builtin_frame_address(0));\n");
        out.push_str("#else\n");
        out.push_str("    py_gc_init(&argc);\n");
        out.push_str("#endif\n");
        out.push_str("    py_set_argv(argc, argv);\n");
        for n in &names {
            out.push_str(&format!("    g_{} = py_none();\n", sanitize(n)));
            out.push_str(&format!("    py_gc_register_root(&g_{});\n", sanitize(n)));
        }
        let body = self.gen_block(&module.body)?;
        out.push_str(&indent(&body, 1));
        out.push_str("    py_shutdown();\n    return 0;\n}\n\n");

        // function definitions
        for i in 0..self.funcs.len() {
            let code = self.gen_function(i)?;
            out.push_str(&code);
            out.push('\n');
        }
        Ok(out)
    }

    fn prescan(&mut self, stmts: &[Stmt]) -> Result<()> {
        for s in stmts {
            match s {
                Stmt::FromImport { module, names } if module == "math" => {
                    for (n, a) in names {
                        let local = a.clone().unwrap_or_else(|| n.clone());
                        if MATH_CONSTS.contains(&n.as_str()) {
                            self.aliases
                                .insert(local, format!("py_math_const(\"{}\")", n));
                        } else if MATH_FUNCS.contains(&n.as_str()) {
                            self.math_short.insert(local);
                        } else {
                            return Err(self.err(format!("math has no attribute '{}'", n)));
                        }
                    }
                }
                Stmt::FromImport { module, names } if module == "sys" => {
                    for (n, a) in names {
                        let local = a.clone().unwrap_or_else(|| n.clone());
                        if n == "argv" {
                            self.aliases.insert(local, "py_sys_argv()".to_string());
                        }
                    }
                }
                Stmt::If { branches, orelse } => {
                    for (_, b) in branches {
                        self.prescan(b)?;
                    }
                    self.prescan(orelse)?;
                }
                Stmt::While { body, orelse, .. } | Stmt::For { body, orelse, .. } => {
                    self.prescan(body)?;
                    self.prescan(orelse)?;
                }
                Stmt::FunctionDef { body, .. } => self.prescan(body)?,
                Stmt::ClassDef { body, .. } => self.prescan(body)?,
                Stmt::Try {
                    body,
                    handlers,
                    orelse,
                    finally,
                } => {
                    self.prescan(body)?;
                    for h in handlers {
                        self.prescan(&h.body)?;
                    }
                    self.prescan(orelse)?;
                    self.prescan(finally)?;
                }
                _ => {}
            }
        }
        Ok(())
    }

    fn err(&self, msg: impl Into<String>) -> CompileError {
        CompileError::new(self.file, 0, 0, msg)
    }

    fn newtmp(&mut self) -> String {
        self.tmp += 1;
        format!("t{}", self.tmp)
    }

    fn resolve(&self, n: &str) -> Option<Slot> {
        for scope in self.scopes.iter().rev() {
            if let Some(c) = scope.get(n) {
                return Some(c.clone());
            }
        }
        if let Some(c) = self.aliases.get(n) {
            return Some(Slot::Plain(c.clone()));
        }
        if self.module_globals.contains(n) {
            return Some(Slot::Plain(format!("g_{}", sanitize(n))));
        }
        None
    }

    fn emit_store(&self, slot: &Slot, value: &str, out: &mut String) {
        match slot {
            Slot::Plain(v) => out.push_str(&format!("{} = {};\n", v, value)),
            Slot::Cell(c) => out.push_str(&format!("py_cell_set({}, {});\n", c, value)),
        }
    }

    fn is_user_name(&self, n: &str) -> bool {
        self.scopes.iter().any(|s| s.contains_key(n))
            || self.aliases.contains_key(n)
            || self.module_globals.contains(n)
    }

    // ---------------- functions ----------------

    fn gen_function(&mut self, idx: usize) -> Result<String> {
        let params: Vec<Param> = self.funcs[idx].params.clone();
        let vararg = self.funcs[idx].vararg.clone();
        let kwarg = self.funcs[idx].kwarg.clone();
        let body = self.funcs[idx].body.clone();
        let cname = self.funcs[idx].cname.clone();
        let fname = self.funcs[idx].name.clone();
        let bases = self.funcs[idx].bases.clone();
        let global_decls = self.funcs[idx].globals.clone();
        let free = self.funcs[idx].free.clone();
        let cells = self.funcs[idx].cells.clone();
        let cell_set: HashSet<String> = cells.iter().cloned().collect();

        let mut scope: HashMap<String, Slot> = HashMap::new();
        let mut pre = String::new();
        // Incoming closure cells occupy the leading argument slots.
        for (i, n) in free.iter().enumerate() {
            let cv = format!("f_{}", sanitize(n));
            pre.push_str(&format!("    Pyo* {} = args[{}];\n", cv, i));
            scope.insert(n.clone(), Slot::Cell(cv));
        }
        let base = free.len();
        let bind_param =
            |pre: &mut String, scope: &mut HashMap<String, Slot>, n: &str, pos: usize| {
                let cv = format!("v_{}", sanitize(n));
                if cell_set.contains(n) {
                    pre.push_str(&format!("    Pyo* {} = py_cell_new(args[{}]);\n", cv, pos));
                    scope.insert(n.to_string(), Slot::Cell(cv));
                } else {
                    pre.push_str(&format!("    Pyo* {} = args[{}];\n", cv, pos));
                    scope.insert(n.to_string(), Slot::Plain(cv));
                }
            };
        for (i, p) in params.iter().enumerate() {
            bind_param(&mut pre, &mut scope, &p.name, base + i);
        }
        let mut pos = base + params.len();
        if let Some(v) = &vararg {
            bind_param(&mut pre, &mut scope, v, pos);
            pos += 1;
        }
        if let Some(k) = &kwarg {
            bind_param(&mut pre, &mut scope, k, pos);
        }

        let mut locals: HashSet<String> = HashSet::new();
        if let FuncBody::Stmts(stmts) = &body {
            gather_locals(stmts, &global_decls, &mut locals);
        } else if let FuncBody::Class(stmts) = &body {
            gather_locals(stmts, &global_decls, &mut locals);
        }
        for p in &params {
            locals.insert(p.name.clone());
        }
        if let Some(v) = &vararg {
            locals.insert(v.clone());
        }
        if let Some(k) = &kwarg {
            locals.insert(k.clone());
        }
        let mut extra: Vec<String> = locals
            .iter()
            .filter(|n| {
                !params.iter().any(|p| &p.name == *n)
                    && vararg.as_deref() != Some(n.as_str())
                    && kwarg.as_deref() != Some(n.as_str())
            })
            .cloned()
            .collect();
        extra.sort();
        for n in extra {
            let cv = format!("v_{}", sanitize(&n));
            if cell_set.contains(&n) {
                pre.push_str(&format!("    Pyo* {} = py_cell_new(py_none());\n", cv));
                scope.insert(n, Slot::Cell(cv));
            } else {
                pre.push_str(&format!("    Pyo* {} = py_none();\n", cv));
                scope.insert(n, Slot::Plain(cv));
            }
        }

        self.scopes.push(scope);
        self.func_depth += 1;
        self.break_flags.push(None);
        let g = match &body {
            FuncBody::Stmts(stmts) => self.gen_block(stmts)?,
            FuncBody::Expr(e) => {
                let mut pre2 = String::new();
                let c = self.gen_expr(e, &mut pre2)?;
                format!("{}    return {};\n", pre2, c)
            }
            FuncBody::Class(stmts) => self.gen_class_body(&fname, &bases, stmts)?,
        };
        self.break_flags.pop();
        self.func_depth -= 1;
        self.scopes.pop();

        let mut s = String::new();
        s.push_str(&format!("static Pyo* {}(Pyo** args, int argc) {{\n", cname));
        s.push_str("    (void)argc;\n");
        s.push_str(&pre);
        s.push_str(&g);
        s.push_str("    return py_none();\n}\n");
        Ok(s)
    }

    // ---------------- statements ----------------

    fn gen_block(&mut self, stmts: &[Stmt]) -> Result<String> {
        let mut out = String::new();
        for s in stmts {
            self.gen_stmt(s, &mut out)?;
        }
        Ok(out)
    }

    fn gen_stmt(&mut self, stmt: &Stmt, out: &mut String) -> Result<()> {
        match stmt {
            Stmt::Expr(e) => {
                let mut pre = String::new();
                let c = self.gen_expr(e, &mut pre)?;
                out.push_str(&pre);
                out.push_str(&format!("(void)({});\n", c));
            }
            Stmt::Assign { targets, value } => {
                let mut pre = String::new();
                let c = self.gen_expr(value, &mut pre)?;
                out.push_str(&pre);
                let t = self.newtmp();
                out.push_str(&format!("Pyo* {} = {};\n", t, c));
                for target in targets {
                    self.gen_assign(target, &t, out)?;
                }
            }
            Stmt::AugAssign { target, op, value } => {
                self.gen_augassign(target, *op, value, out)?;
            }
            Stmt::Return(e) => {
                if self.func_depth == 0 {
                    return Err(self.err("'return' outside of a function"));
                }
                if self.blocked_exit > 0 {
                    return Err(
                        self.err("'return' inside a 'try' with 'finally' is not supported yet")
                    );
                }
                match e {
                    Some(e) => {
                        let mut pre = String::new();
                        let c = self.gen_expr(e, &mut pre)?;
                        out.push_str(&pre);
                        for _ in 0..self.handler_depth {
                            out.push_str("py_exc_pop();\n");
                        }
                        out.push_str(&format!("return {};\n", c));
                    }
                    None => {
                        for _ in 0..self.handler_depth {
                            out.push_str("py_exc_pop();\n");
                        }
                        out.push_str("return py_none();\n");
                    }
                }
            }
            Stmt::If { branches, orelse } => {
                self.gen_if(branches, orelse, out)?;
            }
            Stmt::While { cond, body, orelse } => {
                let flag = if orelse.is_empty() {
                    None
                } else {
                    Some(self.newtmp())
                };
                if let Some(f) = &flag {
                    out.push_str(&format!("int {} = 0;\n", f));
                }
                out.push_str("for (;;) {\n");
                let mut pre = String::new();
                let c = self.gen_expr(cond, &mut pre)?;
                out.push_str(&indent(&pre, 1));
                out.push_str(&format!("    if (!py_truth({})) break;\n", c));
                self.break_flags
                    .push(Some((flag.clone(), self.handler_depth)));
                let b = self.gen_block(body)?;
                self.break_flags.pop();
                out.push_str(&indent(&b, 1));
                out.push_str("}\n");
                if let Some(f) = flag {
                    out.push_str(&format!("if (!{}) {{\n", f));
                    let ob = self.gen_block(orelse)?;
                    out.push_str(&indent(&ob, 1));
                    out.push_str("}\n");
                }
            }
            Stmt::For {
                target,
                iter,
                body,
                orelse,
            } => {
                let it = self.newtmp();
                let item = self.newtmp();
                let flag = if orelse.is_empty() {
                    None
                } else {
                    Some(self.newtmp())
                };
                let mut pre = String::new();
                let c = self.gen_expr(iter, &mut pre)?;
                out.push_str(&pre);
                out.push_str(&format!("Pyo* {} = py_iter({});\n", it, c));
                if let Some(f) = &flag {
                    out.push_str(&format!("int {} = 0;\n", f));
                }
                out.push_str(&format!("Pyo* {};\n", item));
                out.push_str(&format!("while (py_iter_next({}, &{})) {{\n", it, item));
                self.gen_assign(target, &item, out)?;
                self.break_flags
                    .push(Some((flag.clone(), self.handler_depth)));
                let b = self.gen_block(body)?;
                self.break_flags.pop();
                out.push_str(&indent(&b, 1));
                out.push_str("}\n");
                if let Some(f) = flag {
                    out.push_str(&format!("if (!{}) {{\n", f));
                    let ob = self.gen_block(orelse)?;
                    out.push_str(&indent(&ob, 1));
                    out.push_str("}\n");
                }
            }
            Stmt::Break => match self.break_flags.last() {
                Some(Some((flag, entry))) => {
                    if self.blocked_exit > 0 {
                        return Err(
                            self.err("'break' inside a 'try' with 'finally' is not supported yet")
                        );
                    }
                    let entry = *entry;
                    for _ in entry..self.handler_depth {
                        out.push_str("py_exc_pop();\n");
                    }
                    match flag {
                        Some(f) => out.push_str(&format!("{} = 1;\nbreak;\n", f)),
                        None => out.push_str("break;\n"),
                    }
                }
                _ => return Err(self.err("'break' outside of a loop")),
            },
            Stmt::Continue => {
                let entry = match self.break_flags.last() {
                    Some(Some((_, entry))) => *entry,
                    _ => return Err(self.err("'continue' outside of a loop")),
                };
                if self.blocked_exit > 0 {
                    return Err(
                        self.err("'continue' inside a 'try' with 'finally' is not supported yet")
                    );
                }
                for _ in entry..self.handler_depth {
                    out.push_str("py_exc_pop();\n");
                }
                out.push_str("continue;\n");
            }
            Stmt::Pass => {}
            Stmt::FunctionDef { id, name, .. } => {
                self.gen_funcdef(*id, name, out)?;
            }
            Stmt::ClassDef { id, name, .. } => {
                self.gen_classdef(*id, name, out)?;
            }
            Stmt::Global(_) => {}
            Stmt::Assert(e, msg) => {
                let mut pre = String::new();
                let c = self.gen_expr(e, &mut pre)?;
                out.push_str(&pre);
                let m = match msg {
                    Some(m) => {
                        let mut p2 = String::new();
                        let mc = self.gen_expr(m, &mut p2)?;
                        out.push_str(&p2);
                        mc
                    }
                    None => "NULL".to_string(),
                };
                out.push_str(&format!("py_assert(py_truth({}), {});\n", c, m));
            }
            Stmt::Del(targets) => {
                for t in targets {
                    if let Expr::Name(n) = t {
                        if let Some(slot) = self.resolve(n) {
                            self.emit_store(&slot, "py_none()", out);
                        }
                    }
                }
            }
            Stmt::Raise { exc, cause } => {
                if let Some(c) = cause {
                    let mut p = String::new();
                    let cc = self.gen_expr(c, &mut p)?;
                    out.push_str(&p);
                    out.push_str(&format!("(void)({});\n", cc));
                }
                match exc {
                    Some(e) => {
                        let mut p = String::new();
                        let ce = self.gen_expr(e, &mut p)?;
                        out.push_str(&p);
                        out.push_str(&format!("py_raise_value({});\n", ce));
                    }
                    None => out.push_str("py_reraise();\n"),
                }
            }
            Stmt::Try {
                body,
                handlers,
                orelse,
                finally,
            } => {
                self.gen_try(body, handlers, orelse, finally, out)?;
            }
            Stmt::Import { module, alias } => {
                if module != "math" && module != "sys" {
                    return Err(self.err(format!(
                        "module '{}' is not available; only 'math' and 'sys' are supported",
                        module
                    )));
                }
                let _ = alias;
            }
            Stmt::FromImport { module, .. } => {
                if module != "math" && module != "sys" {
                    return Err(self.err(format!(
                        "module '{}' is not available; only 'math' and 'sys' are supported",
                        module
                    )));
                }
            }
        }
        Ok(())
    }

    fn gen_funcdef(&mut self, id: usize, name: &str, out: &mut String) -> Result<()> {
        let idx = *self
            .func_index
            .get(&id)
            .ok_or_else(|| self.err(format!("internal: function '{}' not found", name)))?;
        let expr = self.func_object(idx, out)?;
        let val = self.newtmp();
        out.push_str(&format!("Pyo* {} = {};\n", val, expr));
        let slot = self
            .resolve(name)
            .unwrap_or_else(|| Slot::Plain(format!("g_{}", sanitize(name))));
        self.emit_store(&slot, &val, out);
        Ok(())
    }

    fn gen_classdef(&mut self, id: usize, name: &str, out: &mut String) -> Result<()> {
        let idx = *self
            .func_index
            .get(&id)
            .ok_or_else(|| self.err(format!("internal: class '{}' not found", name)))?;
        let mut pre = String::new();
        let expr = self.func_object(idx, &mut pre)?;
        out.push_str(&pre);
        let cls = self.newtmp();
        out.push_str(&format!("Pyo* {} = py_call({}, NULL, 0);\n", cls, expr));
        self.store_name(name, &cls, out)?;
        Ok(())
    }

    fn gen_class_body(&mut self, name: &str, bases: &[Expr], stmts: &[Stmt]) -> Result<String> {
        let mut out = String::new();
        let base = match bases {
            [] => "NULL".to_string(),
            [b] => {
                let mut pre = String::new();
                let c = self.gen_expr(b, &mut pre)?;
                out.push_str(&pre);
                c
            }
            _ => {
                return Err(self.err("multiple inheritance is not supported".to_string()));
            }
        };
        let cls = self.newtmp();
        out.push_str(&format!(
            "    Pyo* {} = py_class(\"{}\", {});\n",
            cls, name, base
        ));
        for s in stmts {
            match s {
                Stmt::Pass => {}
                Stmt::Expr(e) => {
                    let mut pre = String::new();
                    let c = self.gen_expr(e, &mut pre)?;
                    out.push_str(&pre);
                    if let Expr::Str(_) = e {
                        out.push_str(&format!("    py_class_set({}, \"__doc__\", {});\n", cls, c));
                    } else {
                        out.push_str(&format!("    (void){};\n", c));
                    }
                }
                Stmt::FunctionDef { id, name: mn, .. } => {
                    let idx = *self
                        .func_index
                        .get(id)
                        .ok_or_else(|| self.err(format!("internal: method '{}' not found", mn)))?;
                    let mut pre = String::new();
                    let obj = self.func_object(idx, &mut pre)?;
                    out.push_str(&pre);
                    let mv = self.newtmp();
                    out.push_str(&format!("    Pyo* {} = {};\n", mv, obj));
                    out.push_str(&format!("    py_class_set({}, \"{}\", {});\n", cls, mn, mv));
                    self.store_name(mn, &mv, &mut out)?;
                }
                Stmt::ClassDef { id, name: cn, .. } => {
                    let idx = *self
                        .func_index
                        .get(id)
                        .ok_or_else(|| self.err(format!("internal: class '{}' not found", cn)))?;
                    let mut pre = String::new();
                    let obj = self.func_object(idx, &mut pre)?;
                    out.push_str(&pre);
                    let cv = self.newtmp();
                    out.push_str(&format!("    Pyo* {} = py_call({}, NULL, 0);\n", cv, obj));
                    out.push_str(&format!("    py_class_set({}, \"{}\", {});\n", cls, cn, cv));
                    self.store_name(cn, &cv, &mut out)?;
                }
                Stmt::Assign { targets, value } => {
                    let [Expr::Name(n)] = targets.as_slice() else {
                        return Err(self.err(
                            "only simple name assignments are supported in class bodies"
                                .to_string(),
                        ));
                    };
                    let mut pre = String::new();
                    let c = self.gen_expr(value, &mut pre)?;
                    out.push_str(&pre);
                    out.push_str(&format!("    py_class_set({}, \"{}\", {});\n", cls, n, c));
                    self.store_name(n, &c, &mut out)?;
                }
                _ => {
                    return Err(self.err("unsupported statement in class body".to_string()));
                }
            }
        }
        out.push_str(&format!("    return {};\n", cls));
        Ok(out)
    }

    /// Emit the C expression that builds a function/closure object for `idx`.
    /// Any default-value code is appended to `pre`.
    fn func_object(&mut self, idx: usize, pre: &mut String) -> Result<String> {
        let params: Vec<Param> = self.funcs[idx].params.clone();
        let vararg = self.funcs[idx].vararg.clone();
        let kwarg = self.funcs[idx].kwarg.clone();
        let cname = self.funcs[idx].cname.clone();
        let name = if self.funcs[idx].is_lambda {
            "<lambda>".to_string()
        } else {
            self.funcs[idx].name.clone()
        };
        let free = self.funcs[idx].free.clone();

        let mut param_names: Vec<String> =
            params.iter().map(|p| format!("\"{}\"", p.name)).collect();
        if let Some(v) = &vararg {
            param_names.push(format!("\"{}\"", v));
        }
        if let Some(k) = &kwarg {
            param_names.push(format!("\"{}\"", k));
        }
        let params_lit = if param_names.is_empty() {
            "(const char*[]){NULL}".to_string()
        } else {
            format!("(const char*[]){{{}}}", param_names.join(", "))
        };
        let mut defaults = Vec::new();
        for p in &params {
            if let Some(d) = &p.default {
                defaults.push(self.gen_expr(d, pre)?);
            }
        }
        let defaults_lit = if defaults.is_empty() {
            "(Pyo*[]){NULL}".to_string()
        } else {
            format!("(Pyo*[]){{{}}}", defaults.join(", "))
        };
        let var = if vararg.is_some() { 1 } else { 0 };
        let kw = if kwarg.is_some() { 1 } else { 0 };
        if free.is_empty() {
            return Ok(format!(
                "py_func({}, \"{}\", {}, {}, {}, {}, {}, {})",
                cname,
                name,
                params_lit,
                params.len(),
                defaults_lit,
                defaults.len(),
                var,
                kw
            ));
        }
        let mut captured = Vec::new();
        for n in &free {
            match self.resolve(n) {
                Some(Slot::Cell(c)) => captured.push(c),
                _ => return Err(self.err(format!("closure over '{}' is not supported here", n))),
            }
        }
        Ok(format!(
            "py_closure({}, \"{}\", {}, {}, {}, {}, {}, {}, {}, {})",
            cname,
            name,
            params_lit,
            params.len(),
            defaults_lit,
            defaults.len(),
            var,
            kw,
            compound_array(&captured),
            captured.len()
        ))
    }

    fn store_name(&mut self, name: &str, value: &str, out: &mut String) -> Result<()> {
        let slot = self
            .resolve(name)
            .ok_or_else(|| self.err(format!("internal: no slot for '{}'", name)))?;
        self.emit_store(&slot, value, out);
        Ok(())
    }

    fn gen_try(
        &mut self,
        body: &[Stmt],
        handlers: &[ExceptHandler],
        orelse: &[Stmt],
        finally: &[Stmt],
        out: &mut String,
    ) -> Result<()> {
        let has_finally = !finally.is_empty();
        if has_finally {
            self.blocked_exit += 1;
        }
        let outer = self.handler_depth;
        let n = self.newtmp();

        out.push_str("{\n");
        out.push_str(&format!("    PyHandler _h{};\n", n));
        out.push_str(&format!("    PyHandler _h2{};\n", n));
        out.push_str(&format!("    Pyo* _prev{} = py_exc_current();\n", n));
        out.push_str(&format!("    Pyo* _pe{} = NULL;\n", n));
        out.push_str(&format!("    int _pend{} = 0;\n", n));
        out.push_str(&format!("    if (setjmp(_h{}.jb) == 0) {{\n", n));
        out.push_str(&format!("        py_exc_push(&_h{});\n", n));

        self.handler_depth = outer + 1;
        let bcode = self.gen_block(body)?;
        out.push_str(&indent(&bcode, 2));
        self.handler_depth = outer;
        out.push_str("        py_exc_pop();\n");
        if !orelse.is_empty() {
            let ecode = self.gen_block(orelse)?;
            out.push_str(&indent(&ecode, 2));
        }
        out.push_str("    } else {\n");
        out.push_str(&format!("        Pyo* _e{} = py_exc_current();\n", n));

        if handlers.is_empty() {
            out.push_str(&format!("        _pe{} = _e{};\n", n, n));
            out.push_str(&format!("        _pend{} = 1;\n", n));
        } else {
            out.push_str(&format!("        if (setjmp(_h2{}.jb) == 0) {{\n", n));
            out.push_str(&format!("            py_exc_push(&_h2{});\n", n));
            self.handler_depth = outer + 1;
            let mut first = true;
            let mut bare = false;
            for h in handlers {
                match &h.typ {
                    Some(t) => {
                        let mut p = String::new();
                        let ct = self.gen_expr(t, &mut p)?;
                        let cond = format!("py_exc_matches(_e{}, {})", n, ct);
                        out.push_str(&indent(&p, 3));
                        if first {
                            out.push_str(&format!("            if ({}) {{\n", cond));
                        } else {
                            out.push_str(&format!("            else if ({}) {{\n", cond));
                        }
                        first = false;
                        if let Some(nm) = &h.name {
                            self.store_name(nm, &format!("_e{}", n), out)?;
                        }
                        let hc = self.gen_block(&h.body)?;
                        out.push_str(&indent(&hc, 4));
                        out.push_str("            }\n");
                    }
                    None => {
                        bare = true;
                        if first {
                            out.push_str("            {\n");
                        } else {
                            out.push_str("            else {\n");
                        }
                        first = false;
                        if let Some(nm) = &h.name {
                            self.store_name(nm, &format!("_e{}", n), out)?;
                        }
                        let hc = self.gen_block(&h.body)?;
                        out.push_str(&indent(&hc, 4));
                        out.push_str("            }\n");
                    }
                }
            }
            if !bare {
                out.push_str(&format!(
                    "            else {{ _pe{} = _e{}; _pend{} = 1; }}\n",
                    n, n, n
                ));
            }
            out.push_str("            py_exc_pop();\n");
            self.handler_depth = outer;
            out.push_str("        } else {\n");
            out.push_str(&format!("            _pe{} = py_exc_current();\n", n));
            out.push_str(&format!("            _pend{} = 1;\n", n));
            out.push_str("        }\n");
        }
        out.push_str("    }\n");

        if has_finally {
            self.handler_depth = outer;
            let fcode = self.gen_block(finally)?;
            out.push_str(&indent(&fcode, 1));
        }
        out.push_str(&format!("    py_exc_set(_prev{});\n", n));
        out.push_str(&format!("    if (_pend{}) {{ py_raise(_pe{}); }}\n", n, n));
        out.push_str("}\n");

        if has_finally {
            self.blocked_exit -= 1;
        }
        self.handler_depth = outer;
        Ok(())
    }

    fn gen_if(
        &mut self,
        branches: &[(Expr, Vec<Stmt>)],
        orelse: &[Stmt],
        out: &mut String,
    ) -> Result<()> {
        if branches.is_empty() {
            let b = self.gen_block(orelse)?;
            out.push_str("{\n");
            out.push_str(&indent(&b, 1));
            out.push_str("}\n");
            return Ok(());
        }
        let (cond, body) = &branches[0];
        let mut pre = String::new();
        let c = self.gen_expr(cond, &mut pre)?;
        out.push_str(&pre);
        out.push_str(&format!("if (py_truth({})) {{\n", c));
        let b = self.gen_block(body)?;
        out.push_str(&indent(&b, 1));
        out.push_str("} else {\n");
        let mut nested = String::new();
        self.gen_if(&branches[1..], orelse, &mut nested)?;
        out.push_str(&indent(&nested, 1));
        out.push_str("}\n");
        Ok(())
    }

    fn gen_augassign(
        &mut self,
        target: &Expr,
        op: BinOp,
        value: &Expr,
        out: &mut String,
    ) -> Result<()> {
        let fname = binop_fn(op);
        match target {
            Expr::Name(n) => {
                let slot = self
                    .resolve(n)
                    .ok_or_else(|| self.err(format!("name '{}' is not defined", n)))?;
                let read = read_slot(&slot);
                let mut pre = String::new();
                let c = self.gen_expr(value, &mut pre)?;
                out.push_str(&pre);
                let val = format!("{}({}, {})", fname, read, c);
                self.emit_store(&slot, &val, out);
                Ok(())
            }
            Expr::Subscript(base, key) => {
                let mut pre = String::new();
                let b = self.gen_expr(base, &mut pre)?;
                let k = match &**key {
                    Expr::Slice { lower, upper, step } => {
                        let lo = self.gen_slice_part(lower, &mut pre)?;
                        let up = self.gen_slice_part(upper, &mut pre)?;
                        let st = self.gen_slice_part(step, &mut pre)?;
                        format!("py_slice({}, {}, {})", lo, up, st)
                    }
                    _ => self.gen_expr(key, &mut pre)?,
                };
                let cv = self.gen_expr(value, &mut pre)?;
                let bt = self.newtmp();
                let kt = self.newtmp();
                pre.push_str(&format!("Pyo* {} = {};\n", bt, b));
                pre.push_str(&format!("Pyo* {} = {};\n", kt, k));
                out.push_str(&pre);
                out.push_str(&format!(
                    "py_setitem({}, {}, {}(py_getitem({}, {}), {}));\n",
                    bt, kt, fname, bt, kt, cv
                ));
                Ok(())
            }
            Expr::Attribute(base, name) => {
                let mut pre = String::new();
                let b = self.gen_expr(base, &mut pre)?;
                let c = self.gen_expr(value, &mut pre)?;
                let bt = self.newtmp();
                pre.push_str(&format!("Pyo* {} = {};\n", bt, b));
                out.push_str(&pre);
                out.push_str(&format!(
                    "py_setattr({}, \"{}\", {}(py_getattr({}, \"{}\"), {}));\n",
                    bt, name, fname, bt, name, c
                ));
                Ok(())
            }
            _ => Err(self.err("invalid augmented assignment target")),
        }
    }

    fn gen_slice_part(&mut self, e: &Option<Box<Expr>>, pre: &mut String) -> Result<String> {
        match e {
            Some(e) => self.gen_expr(e, pre),
            None => Ok("py_none()".to_string()),
        }
    }

    fn gen_assign(&mut self, target: &Expr, value: &str, out: &mut String) -> Result<()> {
        match target {
            Expr::Name(n) => {
                let slot = self.resolve(n).ok_or_else(|| {
                    self.err(format!("cannot assign to name '{}' (unknown target)", n))
                })?;
                self.emit_store(&slot, value, out);
                Ok(())
            }
            Expr::Tuple(items) | Expr::List(items) => {
                for (i, item) in items.iter().enumerate() {
                    let elem = format!("py_getitem({}, py_int({}))", value, i);
                    self.gen_assign(item, &elem, out)?;
                }
                Ok(())
            }
            Expr::Subscript(base, key) => {
                let mut pre = String::new();
                let b = self.gen_expr(base, &mut pre)?;
                let k = match &**key {
                    Expr::Slice { lower, upper, step } => {
                        let lo = self.gen_slice_part(lower, &mut pre)?;
                        let up = self.gen_slice_part(upper, &mut pre)?;
                        let st = self.gen_slice_part(step, &mut pre)?;
                        format!("py_slice({}, {}, {})", lo, up, st)
                    }
                    _ => self.gen_expr(key, &mut pre)?,
                };
                let bt = self.newtmp();
                let kt = self.newtmp();
                pre.push_str(&format!("Pyo* {} = {};\n", bt, b));
                pre.push_str(&format!("Pyo* {} = {};\n", kt, k));
                out.push_str(&pre);
                out.push_str(&format!("py_setitem({}, {}, {});\n", bt, kt, value));
                Ok(())
            }
            Expr::Attribute(base, name) => {
                let mut pre = String::new();
                let b = self.gen_expr(base, &mut pre)?;
                let bt = self.newtmp();
                pre.push_str(&format!("Pyo* {} = {};\n", bt, b));
                out.push_str(&pre);
                out.push_str(&format!("py_setattr({}, \"{}\", {});\n", bt, name, value));
                Ok(())
            }
            _ => Err(self.err("cannot assign to this expression")),
        }
    }

    // ---------------- expressions ----------------

    fn gen_expr(&mut self, e: &Expr, pre: &mut String) -> Result<String> {
        match e {
            Expr::Int(v) => Ok(format!("py_int({}LL)", v)),
            Expr::Float(v) => Ok(c_float(*v)),
            Expr::Str(s) => Ok(format!("py_strn({}, {})", c_string(s), s.len())),
            Expr::Bool(b) => Ok(if *b { "py_true()" } else { "py_false()" }.to_string()),
            Expr::None => Ok("py_none()".to_string()),
            Expr::FStr(parts) => self.gen_fstring(parts, pre),
            Expr::Name(n) => {
                if let Some(slot) = self.resolve(n) {
                    return Ok(read_slot(&slot));
                }
                if EXC_TYPES.contains(&n.as_str()) {
                    return Ok(format!("py_exc_ref(\"{}\")", n));
                }
                if FIRSTCLASS.contains(&n.as_str()) {
                    return Ok(format!("py_builtin_ref(\"{}\")", n));
                }
                if BUILTINS.contains(&n.as_str()) || MATH_FUNCS.contains(&n.as_str()) {
                    return Err(self.err(format!("builtin '{}' cannot be used as a value", n)));
                }
                Err(self.err(format!("name '{}' is not defined", n)))
            }
            Expr::BinOp(a, op, b) => {
                let ca = self.gen_expr(a, pre)?;
                let cb = self.gen_expr(b, pre)?;
                let t = self.newtmp();
                pre.push_str(&format!(
                    "Pyo* {} = {}({}, {});\n",
                    t,
                    binop_fn(*op),
                    ca,
                    cb
                ));
                Ok(t)
            }
            Expr::UnaryOp(op, a) => {
                let ca = self.gen_expr(a, pre)?;
                let fname = match op {
                    UnOp::Neg => "py_neg",
                    UnOp::Pos => "py_pos",
                    UnOp::Not => "py_not",
                    UnOp::Invert => "py_invert",
                };
                let t = self.newtmp();
                pre.push_str(&format!("Pyo* {} = {}({});\n", t, fname, ca));
                Ok(t)
            }
            Expr::BoolOp(op, parts) => self.gen_boolop(*op, parts, pre),
            Expr::Compare(left, ops) => self.gen_compare(left, ops, pre),
            Expr::Call {
                func,
                args,
                kwargs,
                star_args,
                dstar,
            } => self.gen_call(func, args, kwargs, star_args, dstar.as_deref(), pre),
            Expr::Attribute(base, name) => self.gen_attribute(base, name, pre),
            Expr::Subscript(base, key) => {
                let cb = self.gen_expr(base, pre)?;
                let ck = match &**key {
                    Expr::Slice { lower, upper, step } => {
                        let lo = self.gen_slice_part(lower, pre)?;
                        let up = self.gen_slice_part(upper, pre)?;
                        let st = self.gen_slice_part(step, pre)?;
                        format!("py_slice({}, {}, {})", lo, up, st)
                    }
                    _ => self.gen_expr(key, pre)?,
                };
                let t = self.newtmp();
                pre.push_str(&format!("Pyo* {} = py_getitem({}, {});\n", t, cb, ck));
                Ok(t)
            }
            Expr::Slice { .. } => Err(self.err("slice is only valid inside a subscript")),
            Expr::List(items) => {
                let arr = self.gen_array(items, pre)?;
                let t = self.newtmp();
                pre.push_str(&format!("Pyo* {} = py_list({}, {});\n", t, arr.0, arr.1));
                Ok(t)
            }
            Expr::Tuple(items) => {
                let arr = self.gen_array(items, pre)?;
                let t = self.newtmp();
                pre.push_str(&format!("Pyo* {} = py_tuple({}, {});\n", t, arr.0, arr.1));
                Ok(t)
            }
            Expr::Set(items) => {
                let arr = self.gen_array(items, pre)?;
                let t = self.newtmp();
                pre.push_str(&format!("Pyo* {} = py_set({}, {});\n", t, arr.0, arr.1));
                Ok(t)
            }
            Expr::Dict(pairs) => self.gen_dict(pairs, pre),
            Expr::IfExp { cond, body, orelse } => {
                let cc = self.gen_expr(cond, pre)?;
                let t = self.newtmp();
                pre.push_str(&format!("Pyo* {};\n", t));
                pre.push_str(&format!("if (py_truth({})) {{\n", cc));
                let mut pb = String::new();
                let cb = self.gen_expr(body, &mut pb)?;
                pre.push_str(&indent(&pb, 1));
                pre.push_str(&format!("    {} = {};\n", t, cb));
                pre.push_str("} else {\n");
                let mut po = String::new();
                let co = self.gen_expr(orelse, &mut po)?;
                pre.push_str(&indent(&po, 1));
                pre.push_str(&format!("    {} = {};\n", t, co));
                pre.push_str("}\n");
                Ok(t)
            }
            Expr::ListComp {
                elt,
                target,
                iter,
                cond,
            } => self.gen_comp(CompKind::List(elt), target, iter, cond.as_deref(), pre),
            Expr::SetComp {
                elt,
                target,
                iter,
                cond,
            } => self.gen_comp(CompKind::Set(elt), target, iter, cond.as_deref(), pre),
            Expr::DictComp {
                key,
                value,
                target,
                iter,
                cond,
            } => self.gen_comp(
                CompKind::Dict(key, value),
                target,
                iter,
                cond.as_deref(),
                pre,
            ),
            Expr::Lambda { id, .. } => {
                let idx = *self
                    .func_index
                    .get(id)
                    .ok_or_else(|| self.err("internal: lambda not found"))?;
                let e = self.func_object(idx, pre)?;
                let t = self.newtmp();
                pre.push_str(&format!("Pyo* {} = {};\n", t, e));
                Ok(t)
            }
        }
    }

    fn gen_array(&mut self, items: &[Expr], pre: &mut String) -> Result<(String, usize)> {
        let mut cs = Vec::new();
        for it in items {
            cs.push(self.gen_expr(it, pre)?);
        }
        Ok((compound_array(&cs), cs.len()))
    }

    fn gen_dict(&mut self, pairs: &[(Option<Expr>, Expr)], pre: &mut String) -> Result<String> {
        let mut keys = Vec::new();
        let mut vals = Vec::new();
        for (k, v) in pairs {
            match k {
                Some(k) => keys.push(self.gen_expr(k, pre)?),
                None => return Err(self.err("dict unpacking is not supported")),
            }
            vals.push(self.gen_expr(v, pre)?);
        }
        let t = self.newtmp();
        if keys.is_empty() {
            pre.push_str(&format!("Pyo* {} = py_dict(NULL, NULL, 0);\n", t));
        } else {
            pre.push_str(&format!(
                "Pyo* {} = py_dict({}, {}, {});\n",
                t,
                compound_array(&keys),
                compound_array(&vals),
                keys.len()
            ));
        }
        Ok(t)
    }

    fn gen_fstring(&mut self, parts: &[FPart], pre: &mut String) -> Result<String> {
        let mut cs = Vec::new();
        for part in parts {
            match part {
                FPart::Lit(s) => cs.push(format!("py_strn({}, {})", c_string(s), s.len())),
                FPart::Expr { src, spec, repr } => {
                    let e = parse_expression(src, self.file)?;
                    let c = self.gen_expr(&e, pre)?;
                    cs.push(format!(
                        "py_format({}, {}, {})",
                        c,
                        c_string(spec),
                        if *repr { 1 } else { 0 }
                    ));
                }
            }
        }
        if cs.is_empty() {
            return Ok("py_str(\"\")".to_string());
        }
        let t = self.newtmp();
        pre.push_str(&format!(
            "Pyo* {} = py_str_concat({}, {});\n",
            t,
            compound_array(&cs),
            cs.len()
        ));
        Ok(t)
    }

    fn gen_boolop(&mut self, op: BoolOp, parts: &[Expr], pre: &mut String) -> Result<String> {
        let t = self.newtmp();
        let first = self.gen_expr(&parts[0], pre)?;
        pre.push_str(&format!("Pyo* {} = {};\n", t, first));
        let test = match op {
            BoolOp::And => "py_truth",
            BoolOp::Or => "!py_truth",
        };
        let mut depth = 0usize;
        for part in &parts[1..] {
            pre.push_str(&format!("if ({}({})) {{\n", test, t));
            depth += 1;
            let mut sub = String::new();
            let c = self.gen_expr(part, &mut sub)?;
            pre.push_str(&indent(&sub, depth));
            pre.push_str(&format!("{} = {};\n", t, c));
        }
        for _ in 0..depth {
            pre.push_str("}\n");
        }
        Ok(t)
    }

    fn gen_compare(
        &mut self,
        left: &Expr,
        ops: &[(CmpOp, Expr)],
        pre: &mut String,
    ) -> Result<String> {
        let mut prev = self.newtmp();
        let first = self.gen_expr(left, pre)?;
        pre.push_str(&format!("Pyo* {} = {};\n", prev, first));
        let ok = self.newtmp();
        pre.push_str(&format!("int {} = 1;\n", ok));
        for (op, right) in ops {
            let cur = self.newtmp();
            pre.push_str(&format!("Pyo* {} = py_none();\n", cur));
            pre.push_str(&format!("if ({}) {{\n", ok));
            let mut sub = String::new();
            let cr = self.gen_expr(right, &mut sub)?;
            sub.push_str(&format!("{} = {};\n", cur, cr));
            let test = match op {
                CmpOp::In => format!("py_truth(py_in({}, {}))", prev, cur),
                CmpOp::NotIn => format!("!py_truth(py_in({}, {}))", prev, cur),
                CmpOp::Is => format!("py_truth(py_is({}, {}))", prev, cur),
                CmpOp::IsNot => format!("py_truth(py_isnot({}, {}))", prev, cur),
                _ => format!("py_truth({}({}, {}))", cmp_fn(*op), prev, cur),
            };
            sub.push_str(&format!("{} = {};\n", ok, test));
            pre.push_str(&indent(&sub, 1));
            pre.push_str("}\n");
            prev = cur;
        }
        let t = self.newtmp();
        pre.push_str(&format!("Pyo* {} = py_bool({});\n", t, ok));
        Ok(t)
    }

    fn gen_attribute(&mut self, base: &Expr, name: &str, pre: &mut String) -> Result<String> {
        if let Expr::Name(m) = base {
            if !self.is_user_name(m) {
                if m == "math" {
                    if MATH_CONSTS.contains(&name) {
                        return Ok(format!("py_math_const(\"{}\")", name));
                    }
                    return Err(self.err(format!("math.{} cannot be used as a value", name)));
                }
                if m == "sys" && name == "argv" {
                    return Ok("py_sys_argv()".to_string());
                }
            }
        }
        let b = self.gen_expr(base, pre)?;
        Ok(format!("py_getattr({}, \"{}\")", b, name))
    }

    fn gen_comp(
        &mut self,
        kind: CompKind,
        target: &Expr,
        iter: &Expr,
        cond: Option<&Expr>,
        pre: &mut String,
    ) -> Result<String> {
        let it = self.newtmp();
        let item = self.newtmp();
        let result = self.newtmp();
        let mut scope = HashMap::new();
        self.bind_names(target, &mut scope);
        let citer = self.gen_expr(iter, pre)?;
        let init = match kind {
            CompKind::List(_) => "py_list(NULL, 0)".to_string(),
            CompKind::Set(_) => "py_set(NULL, 0)".to_string(),
            CompKind::Dict(_, _) => "py_dict(NULL, NULL, 0)".to_string(),
        };
        pre.push_str(&format!("Pyo* {} = {};\n", result, init));
        pre.push_str("{\n");
        pre.push_str(&format!("Pyo* {} = py_iter({});\n", it, citer));
        pre.push_str(&format!("Pyo* {};\n", item));
        pre.push_str(&format!("while (py_iter_next({}, &{})) {{\n", it, item));
        for slot in scope.values() {
            if let Slot::Plain(cv) = slot {
                pre.push_str(&format!("    Pyo* {} = py_none();\n", cv));
            }
        }
        self.scopes.push(scope.clone());
        self.gen_assign(target, &item, pre)?;
        if let Some(cond) = cond {
            let mut cpre = String::new();
            let cc = self.gen_expr(cond, &mut cpre)?;
            pre.push_str(&indent(&cpre, 1));
            pre.push_str(&format!("    if (py_truth({})) {{\n", cc));
            let mut epre = String::new();
            let stmt = self.gen_comp_append(result.as_str(), kind, &mut epre)?;
            pre.push_str(&indent(&epre, 2));
            pre.push_str(&format!("        {}\n", stmt));
            pre.push_str("    }\n");
        } else {
            let mut epre = String::new();
            let stmt = self.gen_comp_append(result.as_str(), kind, &mut epre)?;
            pre.push_str(&indent(&epre, 1));
            pre.push_str(&format!("    {}\n", stmt));
        }
        self.scopes.pop();
        pre.push_str("}\n}\n");
        Ok(result)
    }

    fn gen_comp_append(
        &mut self,
        result: &str,
        kind: CompKind,
        pre: &mut String,
    ) -> Result<String> {
        match kind {
            CompKind::List(elt) => {
                let ce = self.gen_expr(elt, pre)?;
                Ok(format!("py_list_append({}, {});", result, ce))
            }
            CompKind::Set(elt) => {
                let ce = self.gen_expr(elt, pre)?;
                Ok(format!("py_set_add({}, {});", result, ce))
            }
            CompKind::Dict(key, value) => {
                let ck = self.gen_expr(key, pre)?;
                let cv = self.gen_expr(value, pre)?;
                Ok(format!("py_dict_set({}, {}, {});", result, ck, cv))
            }
        }
    }

    fn bind_names(&self, target: &Expr, scope: &mut HashMap<String, Slot>) {
        match target {
            Expr::Name(n) => {
                scope.insert(
                    n.clone(),
                    Slot::Plain(format!("v_{}_{}", sanitize(n), self.tmp)),
                );
            }
            Expr::Tuple(items) | Expr::List(items) => {
                for it in items {
                    self.bind_names(it, scope);
                }
            }
            _ => {}
        }
    }

    fn gen_call(
        &mut self,
        func: &Expr,
        args: &[Expr],
        kwargs: &[(String, Expr)],
        star_args: &[Expr],
        dstar: Option<&Expr>,
        pre: &mut String,
    ) -> Result<String> {
        if let Expr::Name(n) = func {
            if !self.is_user_name(n) {
                if n == "print" && !star_args.is_empty() {
                    return self.gen_dynamic_call(func, args, kwargs, star_args, dstar, pre);
                }
                if n == "print" {
                    return self.gen_print(args, kwargs, star_args, dstar, pre);
                }
                if BUILTINS.contains(&n.as_str()) {
                    return self.gen_builtin(n, args, kwargs, pre);
                }
                if self.math_short.contains(n) {
                    if !star_args.is_empty() || dstar.is_some() || !kwargs.is_empty() {
                        return Err(self.err("math functions do not accept keywords or *args"));
                    }
                    let (arr, count) = self.gen_array(args, pre)?;
                    let t = self.newtmp();
                    pre.push_str(&format!(
                        "Pyo* {} = py_math_call(\"{}\", {}, {});\n",
                        t, n, arr, count
                    ));
                    return Ok(t);
                }
            }
        }
        if let Expr::Attribute(base, name) = func {
            if let Expr::Name(m) = &**base {
                if !self.is_user_name(m) {
                    if m == "math" {
                        if !star_args.is_empty() || dstar.is_some() || !kwargs.is_empty() {
                            return Err(self.err("math functions do not accept keywords or *args"));
                        }
                        let (arr, count) = self.gen_array(args, pre)?;
                        let t = self.newtmp();
                        pre.push_str(&format!(
                            "Pyo* {} = py_math_call(\"{}\", {}, {});\n",
                            t, name, arr, count
                        ));
                        return Ok(t);
                    }
                    if m == "sys" && name == "exit" {
                        let c = if args.is_empty() {
                            "py_none()".to_string()
                        } else {
                            self.gen_expr(&args[0], pre)?
                        };
                        pre.push_str(&format!("py_sys_exit({});\n", c));
                        return Ok("py_none()".to_string());
                    }
                }
            }
            // Regular method call.
            if dstar.is_some() {
                return Err(self.err("**kwargs are not supported for method calls"));
            }
            if star_args.is_empty() && name == "sort" {
                let cb = self.gen_expr(base, pre)?;
                let mut rev = "NULL".to_string();
                let mut key = "NULL".to_string();
                for (k, v) in kwargs {
                    match k.as_str() {
                        "reverse" => rev = self.gen_expr(v, pre)?,
                        "key" => key = self.gen_expr(v, pre)?,
                        other => {
                            return Err(
                                self.err(format!("list.sort() does not support '{}'", other))
                            )
                        }
                    }
                }
                pre.push_str(&format!("py_list_sort({}, {}, {});\n", cb, rev, key));
                return Ok("py_none()".to_string());
            }
            let cb = self.gen_expr(base, pre)?;
            let (arr, count) = if star_args.is_empty() {
                self.gen_array(args, pre)?
            } else {
                return Err(self.err("*args are not supported for method calls"));
            };
            if !kwargs.is_empty() {
                return Err(self.err("keyword arguments are not supported for method calls"));
            }
            let t = self.newtmp();
            pre.push_str(&format!(
                "Pyo* {} = py_method({}, \"{}\", {}, {});\n",
                t, cb, name, arr, count
            ));
            return Ok(t);
        }
        self.gen_dynamic_call(func, args, kwargs, star_args, dstar, pre)
    }

    fn gen_dynamic_call(
        &mut self,
        func: &Expr,
        args: &[Expr],
        kwargs: &[(String, Expr)],
        star_args: &[Expr],
        dstar: Option<&Expr>,
        pre: &mut String,
    ) -> Result<String> {
        let cf = self.gen_expr(func, pre)?;
        if star_args.is_empty() && dstar.is_none() {
            let (arr, count) = self.gen_array(args, pre)?;
            let t = self.newtmp();
            if kwargs.is_empty() {
                pre.push_str(&format!(
                    "Pyo* {} = py_call({}, {}, {});\n",
                    t, cf, arr, count
                ));
            } else {
                let mut names = Vec::new();
                let mut vals = Vec::new();
                for (k, v) in kwargs {
                    names.push(format!("py_str(\"{}\")", k));
                    vals.push(self.gen_expr(v, pre)?);
                }
                pre.push_str(&format!(
                    "Pyo* {} = py_call_kw({}, {}, {}, {}, {}, {});\n",
                    t,
                    cf,
                    arr,
                    count,
                    compound_array(&names),
                    compound_array(&vals),
                    names.len()
                ));
            }
            return Ok(t);
        }
        // Dynamic path with *args / **kwargs.
        let av = self.newtmp();
        let kn = self.newtmp();
        let kv = self.newtmp();
        pre.push_str(&format!("ArgVec {}; py_args_init(&{});\n", av, av));
        for a in args {
            let c = self.gen_expr(a, pre)?;
            pre.push_str(&format!("py_args_push(&{}, {});\n", av, c));
        }
        for a in star_args {
            let c = self.gen_expr(a, pre)?;
            pre.push_str(&format!("py_args_extend(&{}, {});\n", av, c));
        }
        pre.push_str(&format!("ArgVec {}; py_args_init(&{});\n", kn, kn));
        pre.push_str(&format!("ArgVec {}; py_args_init(&{});\n", kv, kv));
        for (k, v) in kwargs {
            pre.push_str(&format!("py_args_push(&{}, py_str(\"{}\"));\n", kn, k));
            let c = self.gen_expr(v, pre)?;
            pre.push_str(&format!("py_args_push(&{}, {});\n", kv, c));
        }
        if let Some(d) = dstar {
            let c = self.gen_expr(d, pre)?;
            pre.push_str(&format!("py_args_extend_kv(&{}, &{}, {});\n", kn, kv, c));
        }
        let t = self.newtmp();
        pre.push_str(&format!(
            "Pyo* {} = py_call_kw({}, {}.items, {}.len, {}.items, {}.items, {}.len);\n",
            t, cf, av, av, kn, kv, kn
        ));
        Ok(t)
    }

    fn gen_print(
        &mut self,
        args: &[Expr],
        kwargs: &[(String, Expr)],
        _star_args: &[Expr],
        dstar: Option<&Expr>,
        pre: &mut String,
    ) -> Result<String> {
        if dstar.is_some() {
            return Err(self.err("**kwargs are not supported in print()"));
        }
        let mut sep = "NULL".to_string();
        let mut end = "NULL".to_string();
        let mut arg_pre = String::new();
        let (arr, count) = self.gen_array(args, &mut arg_pre)?;
        pre.push_str(&arg_pre);
        for (k, v) in kwargs {
            match k.as_str() {
                "sep" => sep = self.gen_expr(v, pre)?,
                "end" => end = self.gen_expr(v, pre)?,
                other => {
                    return Err(self.err(format!("print() got an unexpected keyword '{}'", other)))
                }
            }
        }
        let t = self.newtmp();
        pre.push_str(&format!(
            "Pyo* {} = py_print({}, {}, {}, {});\n",
            t, arr, count, sep, end
        ));
        Ok(t)
    }

    fn gen_builtin(
        &mut self,
        name: &str,
        args: &[Expr],
        kwargs: &[(String, Expr)],
        pre: &mut String,
    ) -> Result<String> {
        let need = |n: usize| -> Result<()> {
            if args.len() != n {
                Err(CompileError::new(
                    self.file,
                    0,
                    0,
                    format!("{}() takes {} argument(s)", name, n),
                ))
            } else {
                Ok(())
            }
        };
        match name {
            "len" => {
                need(1)?;
                let c = self.gen_expr(&args[0], pre)?;
                self.temp(pre, format!("py_len({})", c))
            }
            "str" => {
                if args.is_empty() {
                    return Ok("py_str(\"\")".to_string());
                }
                need(1)?;
                let c = self.gen_expr(&args[0], pre)?;
                self.temp(pre, format!("py_builtin_str({})", c))
            }
            "repr" => {
                need(1)?;
                let c = self.gen_expr(&args[0], pre)?;
                self.temp(pre, format!("py_builtin_repr({})", c))
            }
            "int" | "float" | "bool" | "abs" | "type" => {
                need(1)?;
                let c = self.gen_expr(&args[0], pre)?;
                let f = match name {
                    "int" => "py_builtin_int",
                    "float" => "py_builtin_float",
                    "bool" => "py_builtin_bool",
                    "abs" => "py_abs",
                    _ => "py_type",
                };
                self.temp(pre, format!("{}({})", f, c))
            }
            "chr" | "ord" | "hex" | "oct" | "bin" => {
                need(1)?;
                let c = self.gen_expr(&args[0], pre)?;
                let f = match name {
                    "chr" => "py_chr",
                    "ord" => "py_ord",
                    "hex" => "py_hex",
                    "oct" => "py_oct",
                    _ => "py_bin",
                };
                self.temp(pre, format!("{}({})", f, c))
            }
            "isinstance" => {
                need(2)?;
                let a = self.gen_expr(&args[0], pre)?;
                let b = self.gen_expr(&args[1], pre)?;
                self.temp(pre, format!("py_isinstance({}, {})", a, b))
            }
            "range" | "min" | "max" | "sum" | "any" | "all" | "zip" | "input" => {
                if !kwargs.is_empty() {
                    return Err(self.err(format!("{}() does not accept keyword arguments", name)));
                }
                let (arr, count) = self.gen_array(args, pre)?;
                let f = match name {
                    "range" => "py_range",
                    "min" => "py_min",
                    "max" => "py_max",
                    "sum" => "py_sum",
                    "any" => "py_any",
                    "all" => "py_all",
                    "zip" => "py_zip",
                    _ => "py_input",
                };
                self.temp(pre, format!("{}({}, {})", f, arr, count))
            }
            "reversed" | "list" | "tuple" | "set" | "dict" => {
                need(1)?;
                let c = self.gen_expr(&args[0], pre)?;
                let f = match name {
                    "reversed" => "py_reversed",
                    "list" => "py_list_of",
                    "tuple" => "py_tuple_of",
                    "set" => "py_set_of",
                    _ => "py_dict_of",
                };
                self.temp(pre, format!("{}({})", f, c))
            }
            "sorted" => {
                need(1)?;
                let c = self.gen_expr(&args[0], pre)?;
                let mut rev = "NULL".to_string();
                let mut key = "NULL".to_string();
                for (k, v) in kwargs {
                    match k.as_str() {
                        "reverse" => rev = self.gen_expr(v, pre)?,
                        "key" => key = self.gen_expr(v, pre)?,
                        _ => return Err(self.err(format!("sorted() does not support '{}'", k))),
                    }
                }
                self.temp(pre, format!("py_sorted({}, {}, {})", c, rev, key))
            }
            "enumerate" => {
                let c = self.gen_expr(&args[0], pre)?;
                let start = if args.len() > 1 {
                    self.gen_expr(&args[1], pre)?
                } else {
                    "NULL".to_string()
                };
                self.temp(pre, format!("py_enumerate({}, {})", c, start))
            }
            "round" => {
                let (arr, count) = self.gen_array(args, pre)?;
                self.temp(pre, format!("py_round({}, {})", arr, count))
            }
            "pow" => {
                need(2)?;
                let a = self.gen_expr(&args[0], pre)?;
                let b = self.gen_expr(&args[1], pre)?;
                self.temp(pre, format!("py_pow_builtin({}, {})", a, b))
            }
            "divmod" => {
                need(2)?;
                let a = self.gen_expr(&args[0], pre)?;
                let b = self.gen_expr(&args[1], pre)?;
                self.temp(pre, format!("py_divmod({}, {})", a, b))
            }
            _ => Err(self.err(format!("builtin '{}' is not supported", name))),
        }
    }

    fn temp(&mut self, pre: &mut String, expr: String) -> Result<String> {
        let t = self.newtmp();
        pre.push_str(&format!("Pyo* {} = {};\n", t, expr));
        Ok(t)
    }
}

// ---------------- helpers ----------------

fn sanitize(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

fn compound_array(items: &[String]) -> String {
    if items.is_empty() {
        "(Pyo*[]){NULL}".to_string()
    } else {
        format!("(Pyo*[]){{{}}}", items.join(", "))
    }
}

fn c_float(v: f64) -> String {
    if v.is_nan() {
        "py_float(0.0 / 0.0)".to_string()
    } else if v.is_infinite() {
        if v > 0.0 {
            "py_float(1.0 / 0.0)".to_string()
        } else {
            "py_float(-1.0 / 0.0)".to_string()
        }
    } else if v == v.trunc() && v.abs() < 1e15 {
        format!("py_float({:.1})", v)
    } else {
        format!("py_float({:e})", v)
    }
}

fn c_string(s: &str) -> String {
    let mut out = String::from("\"");
    for b in s.bytes() {
        match b {
            b'"' => out.push_str("\\\""),
            b'\\' => out.push_str("\\\\"),
            b'\n' => out.push_str("\\n"),
            b'\t' => out.push_str("\\t"),
            b'\r' => out.push_str("\\r"),
            0x20..=0x7e => out.push(b as char),
            _ => out.push_str(&format!("\\{:03o}", b)),
        }
    }
    out.push('"');
    out
}

fn binop_fn(op: BinOp) -> &'static str {
    match op {
        BinOp::Add => "py_add",
        BinOp::Sub => "py_sub",
        BinOp::Mul => "py_mul",
        BinOp::Div => "py_div",
        BinOp::FloorDiv => "py_floordiv",
        BinOp::Mod => "py_mod",
        BinOp::Pow => "py_pow",
        BinOp::LShift => "py_lshift",
        BinOp::RShift => "py_rshift",
        BinOp::BitAnd => "py_bitand",
        BinOp::BitOr => "py_bitor",
        BinOp::BitXor => "py_bitxor",
    }
}

fn cmp_fn(op: CmpOp) -> &'static str {
    match op {
        CmpOp::Eq => "py_eq",
        CmpOp::Ne => "py_ne",
        CmpOp::Lt => "py_lt",
        CmpOp::Le => "py_le",
        CmpOp::Gt => "py_gt",
        CmpOp::Ge => "py_ge",
        CmpOp::In => "py_in",
        CmpOp::NotIn => "py_in",
        CmpOp::Is => "py_is",
        CmpOp::IsNot => "py_isnot",
    }
}

fn indent(s: &str, levels: usize) -> String {
    if s.is_empty() {
        return String::new();
    }
    let pad = "    ".repeat(levels);
    s.lines()
        .map(|l| {
            if l.is_empty() {
                String::from("\n")
            } else {
                format!("{}{}\n", pad, l)
            }
        })
        .collect()
}

fn gather_target_names(t: &Expr, out: &mut HashSet<String>) {
    match t {
        Expr::Name(n) => {
            out.insert(n.clone());
        }
        Expr::Tuple(items) | Expr::List(items) => {
            for it in items {
                gather_target_names(it, out);
            }
        }
        _ => {}
    }
}

fn gather_globals(stmts: &[Stmt], out: &mut HashSet<String>) {
    for s in stmts {
        match s {
            Stmt::Assign { targets, .. } => {
                for t in targets {
                    gather_target_names(t, out);
                }
            }
            Stmt::AugAssign { target, .. } => gather_target_names(target, out),
            Stmt::For {
                target,
                body,
                orelse,
                ..
            } => {
                gather_target_names(target, out);
                gather_globals(body, out);
                gather_globals(orelse, out);
            }
            Stmt::While { body, orelse, .. } => {
                gather_globals(body, out);
                gather_globals(orelse, out);
            }
            Stmt::If { branches, orelse } => {
                for (_, b) in branches {
                    gather_globals(b, out);
                }
                gather_globals(orelse, out);
            }
            Stmt::Try {
                body,
                handlers,
                orelse,
                finally,
            } => {
                gather_globals(body, out);
                for h in handlers {
                    if let Some(nm) = &h.name {
                        out.insert(nm.clone());
                    }
                    gather_globals(&h.body, out);
                }
                gather_globals(orelse, out);
                gather_globals(finally, out);
            }
            Stmt::FunctionDef { name, .. } => {
                out.insert(name.clone());
            }
            Stmt::ClassDef { name, .. } => {
                out.insert(name.clone());
            }
            Stmt::Import { module, alias } => {
                if module == "math" || module == "sys" {
                    if let Some(a) = alias {
                        out.insert(a.clone());
                    }
                }
            }
            Stmt::FromImport { module, names } if module != "math" && module != "sys" => {
                for (n, a) in names {
                    out.insert(a.clone().unwrap_or_else(|| n.clone()));
                }
            }
            _ => {}
        }
    }
}

fn gather_locals(stmts: &[Stmt], globals: &HashSet<String>, out: &mut HashSet<String>) {
    for s in stmts {
        match s {
            Stmt::Assign { targets, .. } => {
                for t in targets {
                    let mut names = HashSet::new();
                    gather_target_names(t, &mut names);
                    for n in names {
                        if !globals.contains(&n) {
                            out.insert(n);
                        }
                    }
                }
            }
            Stmt::AugAssign { target, .. } => {
                let mut names = HashSet::new();
                gather_target_names(target, &mut names);
                for n in names {
                    if !globals.contains(&n) {
                        out.insert(n);
                    }
                }
            }
            Stmt::For {
                target,
                body,
                orelse,
                ..
            } => {
                let mut names = HashSet::new();
                gather_target_names(target, &mut names);
                for n in names {
                    if !globals.contains(&n) {
                        out.insert(n);
                    }
                }
                gather_locals(body, globals, out);
                gather_locals(orelse, globals, out);
            }
            Stmt::While { body, orelse, .. } => {
                gather_locals(body, globals, out);
                gather_locals(orelse, globals, out);
            }
            Stmt::If { branches, orelse } => {
                for (_, b) in branches {
                    gather_locals(b, globals, out);
                }
                gather_locals(orelse, globals, out);
            }
            Stmt::Try {
                body,
                handlers,
                orelse,
                finally,
            } => {
                gather_locals(body, globals, out);
                for h in handlers {
                    if let Some(nm) = &h.name {
                        if !globals.contains(nm) {
                            out.insert(nm.clone());
                        }
                    }
                    gather_locals(&h.body, globals, out);
                }
                gather_locals(orelse, globals, out);
                gather_locals(finally, globals, out);
            }
            Stmt::Import { module, alias } => {
                if module == "math" || module == "sys" {
                    if let Some(a) = alias {
                        if !globals.contains(a) {
                            out.insert(a.clone());
                        }
                    }
                }
            }
            Stmt::FromImport { module, names } if module != "math" && module != "sys" => {
                for (n, a) in names {
                    let local = a.clone().unwrap_or_else(|| n.clone());
                    if !globals.contains(&local) {
                        out.insert(local);
                    }
                }
            }
            Stmt::FunctionDef { name, .. } if !globals.contains(name) => {
                out.insert(name.clone());
            }
            Stmt::ClassDef { name, .. } if !globals.contains(name) => {
                out.insert(name.clone());
            }
            _ => {}
        }
    }
}

fn gather_global_decls(stmts: &[Stmt], out: &mut HashSet<String>) {
    for s in stmts {
        match s {
            Stmt::Global(names) => {
                for n in names {
                    out.insert(n.clone());
                }
            }
            Stmt::If { branches, orelse } => {
                for (_, b) in branches {
                    gather_global_decls(b, out);
                }
                gather_global_decls(orelse, out);
            }
            Stmt::While { body, orelse, .. } | Stmt::For { body, orelse, .. } => {
                gather_global_decls(body, out);
                gather_global_decls(orelse, out);
            }
            Stmt::Try {
                body,
                handlers,
                orelse,
                finally,
            } => {
                gather_global_decls(body, out);
                for h in handlers {
                    gather_global_decls(&h.body, out);
                }
                gather_global_decls(orelse, out);
                gather_global_decls(finally, out);
            }
            _ => {}
        }
    }
}

struct RawFunc {
    id: usize,
    name: String,
    is_lambda: bool,
    is_method: bool,
    is_class: bool,
    bases: Vec<Expr>,
    params: Vec<Param>,
    vararg: Option<String>,
    kwarg: Option<String>,
    body: FuncBody,
    parent: Option<usize>,
}

fn collect_all(stmts: &[Stmt], parent: Option<usize>, out: &mut Vec<RawFunc>) {
    for s in stmts {
        collect_stmt(s, parent, out);
    }
}

fn collect_stmt(s: &Stmt, parent: Option<usize>, out: &mut Vec<RawFunc>) {
    match s {
        Stmt::FunctionDef {
            id,
            name,
            params,
            vararg,
            kwarg,
            body,
        } => {
            let idx = out.len();
            out.push(RawFunc {
                id: *id,
                name: name.clone(),
                is_lambda: false,
                is_method: false,
                is_class: false,
                bases: Vec::new(),
                params: params.clone(),
                vararg: vararg.clone(),
                kwarg: kwarg.clone(),
                body: FuncBody::Stmts(body.clone()),
                parent,
            });
            for p in params {
                if let Some(d) = &p.default {
                    collect_expr(d, parent, out);
                }
            }
            collect_all(body, Some(idx), out);
        }
        Stmt::ClassDef {
            id,
            name,
            bases,
            body,
        } => {
            let idx = out.len();
            out.push(RawFunc {
                id: *id,
                name: name.clone(),
                is_lambda: false,
                is_method: false,
                is_class: true,
                bases: bases.clone(),
                params: Vec::new(),
                vararg: None,
                kwarg: None,
                body: FuncBody::Class(body.clone()),
                parent,
            });
            for b in bases {
                collect_expr(b, Some(idx), out);
            }
            collect_class_body(body, idx, out);
        }
        Stmt::Expr(e) => collect_expr(e, parent, out),
        Stmt::Assign { targets, value } => {
            for t in targets {
                collect_expr(t, parent, out);
            }
            collect_expr(value, parent, out);
        }
        Stmt::AugAssign { target, value, .. } => {
            collect_expr(target, parent, out);
            collect_expr(value, parent, out);
        }
        Stmt::Return(Some(e)) => collect_expr(e, parent, out),
        Stmt::Assert(e, msg) => {
            collect_expr(e, parent, out);
            if let Some(m) = msg {
                collect_expr(m, parent, out);
            }
        }
        Stmt::If { branches, orelse } => {
            for (c, b) in branches {
                collect_expr(c, parent, out);
                collect_all(b, parent, out);
            }
            collect_all(orelse, parent, out);
        }
        Stmt::While { cond, body, orelse } => {
            collect_expr(cond, parent, out);
            collect_all(body, parent, out);
            collect_all(orelse, parent, out);
        }
        Stmt::For {
            target,
            iter,
            body,
            orelse,
        } => {
            collect_expr(target, parent, out);
            collect_expr(iter, parent, out);
            collect_all(body, parent, out);
            collect_all(orelse, parent, out);
        }
        Stmt::Del(targets) => {
            for t in targets {
                collect_expr(t, parent, out);
            }
        }
        Stmt::Raise { exc, cause } => {
            if let Some(e) = exc {
                collect_expr(e, parent, out);
            }
            if let Some(c) = cause {
                collect_expr(c, parent, out);
            }
        }
        Stmt::Try {
            body,
            handlers,
            orelse,
            finally,
        } => {
            collect_all(body, parent, out);
            for h in handlers {
                if let Some(t) = &h.typ {
                    collect_expr(t, parent, out);
                }
                collect_all(&h.body, parent, out);
            }
            collect_all(orelse, parent, out);
            collect_all(finally, parent, out);
        }
        _ => {}
    }
}

fn collect_class_body(body: &[Stmt], class_idx: usize, out: &mut Vec<RawFunc>) {
    for s in body {
        match s {
            Stmt::FunctionDef { .. } => {
                let before = out.len();
                collect_stmt(s, Some(class_idx), out);
                out[before].is_method = true;
            }
            Stmt::ClassDef { .. } => collect_stmt(s, Some(class_idx), out),
            Stmt::Assign { value, .. } => collect_expr(value, Some(class_idx), out),
            Stmt::AugAssign { target, value, .. } => {
                collect_expr(target, Some(class_idx), out);
                collect_expr(value, Some(class_idx), out);
            }
            Stmt::Expr(e) => collect_expr(e, Some(class_idx), out),
            _ => {}
        }
    }
}

fn collect_expr(e: &Expr, parent: Option<usize>, out: &mut Vec<RawFunc>) {
    match e {
        Expr::Lambda {
            id,
            params,
            vararg,
            kwarg,
            body,
        } => {
            let idx = out.len();
            out.push(RawFunc {
                id: *id,
                name: "<lambda>".to_string(),
                is_lambda: true,
                is_method: false,
                is_class: false,
                bases: Vec::new(),
                params: params.clone(),
                vararg: vararg.clone(),
                kwarg: kwarg.clone(),
                body: FuncBody::Expr((**body).clone()),
                parent,
            });
            for p in params {
                if let Some(d) = &p.default {
                    collect_expr(d, parent, out);
                }
            }
            collect_expr(body, Some(idx), out);
        }
        Expr::BinOp(a, _, b) => {
            collect_expr(a, parent, out);
            collect_expr(b, parent, out);
        }
        Expr::UnaryOp(_, a) => collect_expr(a, parent, out),
        Expr::BoolOp(_, ps) => {
            for p in ps {
                collect_expr(p, parent, out);
            }
        }
        Expr::Compare(l, ops) => {
            collect_expr(l, parent, out);
            for (_, r) in ops {
                collect_expr(r, parent, out);
            }
        }
        Expr::Call {
            func,
            args,
            kwargs,
            star_args,
            dstar,
        } => {
            collect_expr(func, parent, out);
            for a in args {
                collect_expr(a, parent, out);
            }
            for (_, v) in kwargs {
                collect_expr(v, parent, out);
            }
            for a in star_args {
                collect_expr(a, parent, out);
            }
            if let Some(d) = dstar {
                collect_expr(d, parent, out);
            }
        }
        Expr::Attribute(b, _) => collect_expr(b, parent, out),
        Expr::Subscript(b, k) => {
            collect_expr(b, parent, out);
            collect_expr(k, parent, out);
        }
        Expr::Slice { lower, upper, step } => {
            for p in [lower, upper, step].into_iter().flatten() {
                collect_expr(p, parent, out);
            }
        }
        Expr::List(items) | Expr::Tuple(items) | Expr::Set(items) => {
            for it in items {
                collect_expr(it, parent, out);
            }
        }
        Expr::Dict(pairs) => {
            for (k, v) in pairs {
                if let Some(k) = k {
                    collect_expr(k, parent, out);
                }
                collect_expr(v, parent, out);
            }
        }
        Expr::IfExp { cond, body, orelse } => {
            collect_expr(cond, parent, out);
            collect_expr(body, parent, out);
            collect_expr(orelse, parent, out);
        }
        Expr::ListComp {
            elt,
            target,
            iter,
            cond,
        } => {
            collect_expr(elt, parent, out);
            collect_expr(target, parent, out);
            collect_expr(iter, parent, out);
            if let Some(c) = cond {
                collect_expr(c, parent, out);
            }
        }
        Expr::SetComp {
            elt,
            target,
            iter,
            cond,
        } => {
            collect_expr(elt, parent, out);
            collect_expr(target, parent, out);
            collect_expr(iter, parent, out);
            if let Some(c) = cond {
                collect_expr(c, parent, out);
            }
        }
        Expr::DictComp {
            key,
            value,
            target,
            iter,
            cond,
        } => {
            collect_expr(key, parent, out);
            collect_expr(value, parent, out);
            collect_expr(target, parent, out);
            collect_expr(iter, parent, out);
            if let Some(c) = cond {
                collect_expr(c, parent, out);
            }
        }
        _ => {}
    }
}

/// Direct (non-transitive) name loads: stops at nested function/lambda bodies,
/// but includes default-value expressions evaluated in the current scope.
fn direct_expr_loads(e: &Expr, out: &mut HashSet<String>, file: &str) {
    match e {
        Expr::Name(n) => {
            out.insert(n.clone());
        }
        Expr::Lambda { params, body, .. } => {
            let _ = body;
            for p in params {
                if let Some(d) = &p.default {
                    direct_expr_loads(d, out, file);
                }
            }
        }
        Expr::BinOp(a, _, b) => {
            direct_expr_loads(a, out, file);
            direct_expr_loads(b, out, file);
        }
        Expr::UnaryOp(_, a) => direct_expr_loads(a, out, file),
        Expr::BoolOp(_, ps) => {
            for p in ps {
                direct_expr_loads(p, out, file);
            }
        }
        Expr::Compare(l, ops) => {
            direct_expr_loads(l, out, file);
            for (_, r) in ops {
                direct_expr_loads(r, out, file);
            }
        }
        Expr::Call {
            func,
            args,
            kwargs,
            star_args,
            dstar,
        } => {
            direct_expr_loads(func, out, file);
            for a in args {
                direct_expr_loads(a, out, file);
            }
            for (_, v) in kwargs {
                direct_expr_loads(v, out, file);
            }
            for a in star_args {
                direct_expr_loads(a, out, file);
            }
            if let Some(d) = dstar {
                direct_expr_loads(d, out, file);
            }
        }
        Expr::Attribute(b, _) => direct_expr_loads(b, out, file),
        Expr::Subscript(b, k) => {
            direct_expr_loads(b, out, file);
            direct_expr_loads(k, out, file);
        }
        Expr::Slice { lower, upper, step } => {
            for p in [lower, upper, step].into_iter().flatten() {
                direct_expr_loads(p, out, file);
            }
        }
        Expr::List(items) | Expr::Tuple(items) | Expr::Set(items) => {
            for it in items {
                direct_expr_loads(it, out, file);
            }
        }
        Expr::Dict(pairs) => {
            for (k, v) in pairs {
                if let Some(k) = k {
                    direct_expr_loads(k, out, file);
                }
                direct_expr_loads(v, out, file);
            }
        }
        Expr::IfExp { cond, body, orelse } => {
            direct_expr_loads(cond, out, file);
            direct_expr_loads(body, out, file);
            direct_expr_loads(orelse, out, file);
        }
        Expr::ListComp {
            elt,
            target,
            iter,
            cond,
        } => {
            direct_expr_loads(iter, out, file);
            let mut names = HashSet::new();
            gather_target_names(target, &mut names);
            let mut inner = HashSet::new();
            direct_expr_loads(elt, &mut inner, file);
            if let Some(c) = cond {
                direct_expr_loads(c, &mut inner, file);
            }
            for n in inner {
                if !names.contains(&n) {
                    out.insert(n);
                }
            }
        }
        Expr::SetComp {
            elt,
            target,
            iter,
            cond,
        } => {
            direct_expr_loads(iter, out, file);
            let mut names = HashSet::new();
            gather_target_names(target, &mut names);
            let mut inner = HashSet::new();
            direct_expr_loads(elt, &mut inner, file);
            if let Some(c) = cond {
                direct_expr_loads(c, &mut inner, file);
            }
            for n in inner {
                if !names.contains(&n) {
                    out.insert(n);
                }
            }
        }
        Expr::DictComp {
            key,
            value,
            target,
            iter,
            cond,
        } => {
            direct_expr_loads(iter, out, file);
            let mut names = HashSet::new();
            gather_target_names(target, &mut names);
            let mut inner = HashSet::new();
            direct_expr_loads(key, &mut inner, file);
            direct_expr_loads(value, &mut inner, file);
            if let Some(c) = cond {
                direct_expr_loads(c, &mut inner, file);
            }
            for n in inner {
                if !names.contains(&n) {
                    out.insert(n);
                }
            }
        }
        Expr::FStr(parts) => {
            for part in parts {
                if let FPart::Expr { src, .. } = part {
                    if let Ok(e) = parse_expression(src, file) {
                        direct_expr_loads(&e, out, file);
                    }
                }
            }
        }
        _ => {}
    }
}

fn direct_stmt_loads(stmts: &[Stmt], out: &mut HashSet<String>, file: &str) {
    for s in stmts {
        match s {
            Stmt::FunctionDef { params, .. } => {
                for p in params {
                    if let Some(d) = &p.default {
                        direct_expr_loads(d, out, file);
                    }
                }
            }
            Stmt::Expr(e) => direct_expr_loads(e, out, file),
            Stmt::Assign { targets, value } => {
                for t in targets {
                    target_loads(t, out, file);
                }
                direct_expr_loads(value, out, file);
            }
            Stmt::AugAssign { target, value, .. } => {
                target_loads(target, out, file);
                direct_expr_loads(value, out, file);
            }
            Stmt::Return(Some(e)) => direct_expr_loads(e, out, file),
            Stmt::Assert(e, msg) => {
                direct_expr_loads(e, out, file);
                if let Some(m) = msg {
                    direct_expr_loads(m, out, file);
                }
            }
            Stmt::If { branches, orelse } => {
                for (c, b) in branches {
                    direct_expr_loads(c, out, file);
                    direct_stmt_loads(b, out, file);
                }
                direct_stmt_loads(orelse, out, file);
            }
            Stmt::While { cond, body, orelse } => {
                direct_expr_loads(cond, out, file);
                direct_stmt_loads(body, out, file);
                direct_stmt_loads(orelse, out, file);
            }
            Stmt::For {
                target,
                iter,
                body,
                orelse,
            } => {
                target_loads(target, out, file);
                direct_expr_loads(iter, out, file);
                direct_stmt_loads(body, out, file);
                direct_stmt_loads(orelse, out, file);
            }
            Stmt::Del(targets) => {
                for t in targets {
                    target_loads(t, out, file);
                }
            }
            Stmt::Raise { exc, cause } => {
                if let Some(e) = exc {
                    direct_expr_loads(e, out, file);
                }
                if let Some(c) = cause {
                    direct_expr_loads(c, out, file);
                }
            }
            Stmt::Try {
                body,
                handlers,
                orelse,
                finally,
            } => {
                direct_stmt_loads(body, out, file);
                for h in handlers {
                    if let Some(t) = &h.typ {
                        direct_expr_loads(t, out, file);
                    }
                    direct_stmt_loads(&h.body, out, file);
                }
                direct_stmt_loads(orelse, out, file);
                direct_stmt_loads(finally, out, file);
            }
            _ => {}
        }
    }
}

fn target_loads(t: &Expr, out: &mut HashSet<String>, file: &str) {
    match t {
        Expr::Name(_) => {}
        Expr::Tuple(items) | Expr::List(items) => {
            for it in items {
                target_loads(it, out, file);
            }
        }
        Expr::Subscript(b, k) => {
            direct_expr_loads(b, out, file);
            direct_expr_loads(k, out, file);
        }
        Expr::Attribute(b, _) => direct_expr_loads(b, out, file),
        _ => {}
    }
}

fn compute_frees(
    funcs: &mut [FuncDef],
    globals: &HashSet<String>,
    aliases: &HashMap<String, String>,
    math_short: &HashSet<String>,
) {
    let n = funcs.len();
    let file = "";
    let mut locals_sets: Vec<HashSet<String>> = Vec::with_capacity(n);
    let mut direct: Vec<HashSet<String>> = Vec::with_capacity(n);
    for f in funcs.iter() {
        let mut loc = HashSet::new();
        let mut dl = HashSet::new();
        match &f.body {
            FuncBody::Stmts(stmts) => {
                gather_locals(stmts, &f.globals, &mut loc);
                direct_stmt_loads(stmts, &mut dl, file);
            }
            FuncBody::Expr(e) => {
                direct_expr_loads(e, &mut dl, file);
            }
            FuncBody::Class(stmts) => {
                gather_locals(stmts, &f.globals, &mut loc);
                for b in &f.bases {
                    direct_expr_loads(b, &mut dl, file);
                }
                direct_stmt_loads(stmts, &mut dl, file);
            }
        }
        for p in &f.params {
            loc.insert(p.name.clone());
        }
        if let Some(v) = &f.vararg {
            loc.insert(v.clone());
        }
        if let Some(k) = &f.kwarg {
            loc.insert(k.clone());
        }
        locals_sets.push(loc);
        direct.push(dl);
    }
    let mut children: Vec<Vec<usize>> = vec![Vec::new(); n];
    for (i, f) in funcs.iter().enumerate() {
        if let Some(p) = f.parent {
            children[p].push(i);
        }
    }
    let mut ancestor_locals: Vec<HashSet<String>> = vec![HashSet::new(); n];
    for i in 0..n {
        let mut acc = HashSet::new();
        let mut cur = funcs[i].parent;
        while let Some(a) = cur {
            acc.extend(locals_sets[a].iter().cloned());
            cur = funcs[a].parent;
        }
        ancestor_locals[i] = acc;
    }
    let mut free: Vec<Vec<String>> = vec![Vec::new(); n];
    for i in (0..n).rev() {
        let mut names: HashSet<String> = direct[i].clone();
        for c in &children[i] {
            for nm in &free[*c] {
                names.insert(nm.clone());
            }
        }
        let mut v: Vec<String> = names
            .into_iter()
            .filter(|nm| {
                if locals_sets[i].contains(nm) {
                    return false;
                }
                if ancestor_locals[i].contains(nm) {
                    return true;
                }
                !globals.contains(nm)
                    && !aliases.contains_key(nm)
                    && !math_short.contains(nm)
                    && !BUILTINS.contains(&nm.as_str())
                    && !EXC_TYPES.contains(&nm.as_str())
            })
            .collect();
        v.sort();
        free[i] = v;
    }
    for i in 0..n {
        let fh = free[i].clone();
        let mut cur = funcs[i].parent;
        while let Some(a) = cur {
            for nm in &fh {
                if locals_sets[a].contains(nm) && !funcs[a].cells.contains(nm) {
                    funcs[a].cells.push(nm.clone());
                }
            }
            cur = funcs[a].parent;
        }
    }
    for i in 0..n {
        funcs[i].free = free[i].clone();
        funcs[i].cells.sort();
    }
}
