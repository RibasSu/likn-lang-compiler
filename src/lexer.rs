use crate::error::CompileError;
use crate::token::{Token, TokenKind};

pub struct Lexer<'a> {
    _src: &'a str,
    chars: Vec<char>,
    pos: usize,
    line: usize,
    column: usize,
}

impl<'a> Lexer<'a> {
    pub fn new(src: &'a str) -> Self {
        Self {
            _src: src,
            chars: src.chars().collect(),
            pos: 0,
            line: 1,
            column: 1,
        }
    }

    pub fn lex(mut self) -> Result<Vec<Token>, CompileError> {
        let mut tokens = Vec::new();

        while let Some(ch) = self.peek_char() {
            if ch.is_whitespace() {
                self.consume_whitespace();
                continue;
            }

            if ch == '/' && self.peek_next_char() == Some('/') {
                self.consume_comment();
                continue;
            }

            if ch == '#' {
                self.consume_comment();
                continue;
            }

            if ch.is_ascii_digit() {
                tokens.push(self.lex_number()?);
                continue;
            }

            if ch == '"' {
                tokens.push(self.lex_string()?);
                continue;
            }

            if ch == '\'' {
                tokens.push(self.lex_char()?);
                continue;
            }

            if is_ident_start(ch) {
                tokens.push(self.lex_ident());
                continue;
            }

            tokens.push(self.lex_symbol()?);
        }

        tokens.push(Token {
            kind: TokenKind::Eof,
            lexeme: String::new(),
            line: self.line,
            column: self.column,
        });

        Ok(tokens)
    }

    fn peek_char(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }

    fn peek_next_char(&self) -> Option<char> {
        self.chars.get(self.pos + 1).copied()
    }

    fn advance_char(&mut self) -> Option<char> {
        let ch = self.peek_char()?;
        self.pos += 1;

        if ch == '\n' {
            self.line += 1;
            self.column = 1;
        } else {
            self.column += 1;
        }

        Some(ch)
    }

    fn consume_whitespace(&mut self) {
        while matches!(self.peek_char(), Some(ch) if ch.is_whitespace()) {
            let _ = self.advance_char();
        }
    }

    fn consume_comment(&mut self) {
        while let Some(ch) = self.peek_char() {
            let _ = self.advance_char();
            if ch == '\n' {
                break;
            }
        }
    }

    fn lex_number(&mut self) -> Result<Token, CompileError> {
        let start_line = self.line;
        let start_column = self.column;
        let mut number = String::new();

        while matches!(self.peek_char(), Some(ch) if ch.is_ascii_digit()) {
            if let Some(ch) = self.advance_char() {
                number.push(ch);
            }
        }

        if self.peek_char() == Some('.')
            && matches!(self.peek_next_char(), Some(ch) if ch.is_ascii_digit())
        {
            if let Some(ch) = self.advance_char() {
                number.push(ch);
            }
            while matches!(self.peek_char(), Some(ch) if ch.is_ascii_digit()) {
                if let Some(ch) = self.advance_char() {
                    number.push(ch);
                }
            }

            if number.parse::<f64>().is_err() {
                return Err(CompileError::new(
                    format!("float inválido: {number}"),
                    start_line,
                    start_column,
                )
                .with_span(number.chars().count())
                .with_label("literal float inválido"));
            }
        } else if number.parse::<i128>().is_err() {
            return Err(CompileError::new(
                format!("inteiro inválido: {number}"),
                start_line,
                start_column,
            )
            .with_span(number.chars().count())
            .with_label("literal inteiro inválido"));
        }

        Ok(Token {
            kind: TokenKind::Number,
            lexeme: number,
            line: start_line,
            column: start_column,
        })
    }

    fn lex_string(&mut self) -> Result<Token, CompileError> {
        let start_line = self.line;
        let start_column = self.column;
        let mut out = String::new();

        let _ = self.advance_char();

        while let Some(ch) = self.peek_char() {
            if ch == '"' {
                let _ = self.advance_char();
                return Ok(Token {
                    kind: TokenKind::String,
                    lexeme: out,
                    line: start_line,
                    column: start_column,
                });
            }

            if ch == '\\' {
                let _ = self.advance_char();
                let escaped = self.advance_char().ok_or_else(|| {
                    CompileError::new("escape incompleto em string", start_line, start_column)
                        .with_label("sequência de escape inicia aqui")
                        .with_help("adicione o caractere do escape após '\\'")
                })?;
                out.push(parse_escape(
                    escaped,
                    self.line,
                    self.column.saturating_sub(1),
                )?);
                continue;
            }

            out.push(ch);
            let _ = self.advance_char();
        }

        Err(
            CompileError::new("string não terminada", start_line, start_column)
                .with_label("string começa aqui")
                .with_help("adicione aspas duplas (\") para fechar a string"),
        )
    }

    fn lex_char(&mut self) -> Result<Token, CompileError> {
        let start_line = self.line;
        let start_column = self.column;

        let _ = self.advance_char();

        let value = match self.peek_char() {
            Some('\\') => {
                let _ = self.advance_char();
                let escaped = self.advance_char().ok_or_else(|| {
                    CompileError::new("escape incompleto em char", start_line, start_column)
                        .with_label("literal char começa aqui")
                })?;
                parse_escape(escaped, self.line, self.column.saturating_sub(1))?
            }
            Some('\'') => {
                return Err(
                    CompileError::new("literal char vazio", start_line, start_column)
                        .with_label("adicione um caractere entre aspas simples"),
                );
            }
            Some(ch) => {
                let _ = self.advance_char();
                ch
            }
            None => {
                return Err(CompileError::new(
                    "literal char não terminado",
                    start_line,
                    start_column,
                )
                .with_label("literal char começa aqui"));
            }
        };

        if self.peek_char() != Some('\'') {
            return Err(CompileError::new(
                "literal char deve conter exatamente um caractere",
                start_line,
                start_column,
            )
            .with_label("feche com aspas simples"));
        }
        let _ = self.advance_char();

        Ok(Token {
            kind: TokenKind::Char,
            lexeme: value.to_string(),
            line: start_line,
            column: start_column,
        })
    }

    fn lex_ident(&mut self) -> Token {
        let start_line = self.line;
        let start_column = self.column;
        let mut ident = String::new();

        while matches!(self.peek_char(), Some(ch) if is_ident_part(ch)) {
            if let Some(ch) = self.advance_char() {
                ident.push(ch);
            }
        }

        Token {
            kind: TokenKind::Ident,
            lexeme: ident,
            line: start_line,
            column: start_column,
        }
    }

    fn lex_symbol(&mut self) -> Result<Token, CompileError> {
        let start_line = self.line;
        let start_column = self.column;
        let first = self.advance_char().ok_or_else(|| {
            CompileError::new("fim inesperado", start_line, start_column)
                .with_label("entrada terminou aqui")
        })?;

        let two_char = match (first, self.peek_char()) {
            ('=', Some('=')) => Some("=="),
            ('!', Some('=')) => Some("!="),
            ('>', Some('=')) => Some(">="),
            ('<', Some('=')) => Some("<="),
            ('&', Some('&')) => Some("&&"),
            ('|', Some('|')) => Some("||"),
            ('-', Some('>')) => Some("->"),
            _ => None,
        };

        if let Some(op) = two_char {
            let _ = self.advance_char();
            return Ok(Token {
                kind: TokenKind::Symbol,
                lexeme: op.to_string(),
                line: start_line,
                column: start_column,
            });
        }

        if "(){}.,:;+*/-%!=<>".contains(first) {
            return Ok(Token {
                kind: TokenKind::Symbol,
                lexeme: first.to_string(),
                line: start_line,
                column: start_column,
            });
        }

        Err(CompileError::new(
            format!("símbolo inesperado: {first}"),
            start_line,
            start_column,
        )
        .with_label("token inválido")
        .with_help("remova o símbolo ou substitua por um operador válido"))
    }
}

fn parse_escape(ch: char, line: usize, column: usize) -> Result<char, CompileError> {
    let mapped = match ch {
        'n' => '\n',
        't' => '\t',
        '\'' => '\'',
        '"' => '"',
        '\\' => '\\',
        _ => {
            return Err(
                CompileError::new(format!("escape inválido: \\{ch}"), line, column)
                    .with_span(2)
                    .with_label("escape não reconhecido")
                    .with_help("escapes válidos: \\n, \\t, \\\\', \\\", \\\\"),
            );
        }
    };
    Ok(mapped)
}

fn is_ident_start(ch: char) -> bool {
    ch.is_ascii_alphabetic() || ch == '_'
}

fn is_ident_part(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || ch == '_'
}
