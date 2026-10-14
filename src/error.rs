use std::fmt;

/// A compile-time error with source location information.
#[derive(Debug, Clone)]
pub struct CompileError {
    pub file: String,
    pub line: usize,
    pub col: usize,
    pub msg: String,
    /// True when the error is a command-line usage error (exit status 2).
    pub usage: bool,
}

impl CompileError {
    pub fn new(file: &str, line: usize, col: usize, msg: impl Into<String>) -> Self {
        CompileError {
            file: file.to_string(),
            line,
            col,
            msg: msg.into(),
            usage: false,
        }
    }

    /// A command-line usage error. Printed as a plain message and exits with status 2.
    pub fn usage(msg: impl Into<String>) -> Self {
        CompileError {
            file: String::new(),
            line: 0,
            col: 0,
            msg: msg.into(),
            usage: true,
        }
    }
}

impl fmt::Display for CompileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.usage {
            write!(f, "pycompile: {}", self.msg)
        } else {
            write!(
                f,
                "{}:{}:{}: error: {}",
                self.file, self.line, self.col, self.msg
            )
        }
    }
}

impl std::error::Error for CompileError {}

pub type Result<T> = std::result::Result<T, CompileError>;
