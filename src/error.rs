use std::fmt;

#[derive(Debug, Clone)]
pub struct CompileError {
    message: String,
    line: usize,
    column: usize,
}

impl CompileError {
    pub fn new(message: impl Into<String>, line: usize, column: usize) -> Self {
        Self {
            message: message.into(),
            line,
            column,
        }
    }
}

impl fmt::Display for CompileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "erro em linha {}, coluna {}: {}",
            self.line, self.column, self.message
        )
    }
}

impl std::error::Error for CompileError {}
