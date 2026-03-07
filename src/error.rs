use std::fmt;

#[derive(Debug, Clone)]
pub struct CompileError {
    pub message: String,
    pub line: usize,
    pub column: usize,
    pub span_len: usize,
    pub label: Option<String>,
    pub help: Option<String>,
    pub file: Option<String>,
    pub source: Option<String>,
}

impl CompileError {
    pub fn new(message: impl Into<String>, line: usize, column: usize) -> Self {
        Self {
            message: message.into(),
            line,
            column,
            span_len: 1,
            label: None,
            help: None,
            file: None,
            source: None,
        }
    }

    pub fn with_span(mut self, span_len: usize) -> Self {
        self.span_len = span_len.max(1);
        self
    }

    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    pub fn with_help(mut self, help: impl Into<String>) -> Self {
        self.help = Some(help.into());
        self
    }

    pub fn with_source_context(
        mut self,
        file: impl Into<String>,
        source: impl Into<String>,
    ) -> Self {
        self.file = Some(file.into());
        self.source = Some(source.into());
        self
    }

    fn fallback_message(&self) -> String {
        format!(
            "error: {} (linha {}, coluna {})",
            self.message, self.line, self.column
        )
    }

    fn render_diagnostic(&self) -> Option<String> {
        let source = self.source.as_ref()?;
        let file = self.file.as_deref().unwrap_or("<input>");
        let line_idx = self.line.checked_sub(1)?;
        let line_text = source.lines().nth(line_idx)?;
        let gutter = self.line.to_string().len();
        let pointer_offset = self.column.saturating_sub(1);
        let pointer = format!(
            "{}{}",
            " ".repeat(pointer_offset),
            "^".to_string() + &"~".repeat(self.span_len.saturating_sub(1))
        );
        let label = self
            .label
            .as_ref()
            .map(|value| format!(" {value}"))
            .unwrap_or_default();

        let mut out = String::new();
        out.push_str(&format!("error: {}\n", self.message));
        out.push_str(&format!(" --> {file}:{}:{}\n", self.line, self.column));
        out.push_str(&format!("{:>gutter$} |\n", ""));
        out.push_str(&format!("{:>gutter$} | {}\n", self.line, line_text));
        out.push_str(&format!("{:>gutter$} | {}{}\n", "", pointer, label));
        if let Some(help) = &self.help {
            out.push_str(&format!("help: {help}\n"));
        }
        Some(out)
    }
}

impl fmt::Display for CompileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(formatted) = self.render_diagnostic() {
            write!(f, "{}", formatted.trim_end())
        } else {
            write!(f, "{}", self.fallback_message())
        }
    }
}

impl std::error::Error for CompileError {}
