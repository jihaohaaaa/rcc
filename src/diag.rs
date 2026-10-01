use crate::span::Span;
use codespan_reporting::diagnostic::{Diagnostic, Label};
use codespan_reporting::files::SimpleFiles;
use codespan_reporting::term;
use codespan_reporting::term::termcolor::{ColorChoice, StandardStream};
use std::fmt;

#[derive(Debug, Clone)]
pub struct DiagMessage {
    pub message: String,
    pub span: Span,
    pub is_error: bool,
}

#[derive(Debug, Default)]
pub struct Diagnostics {
    pub messages: Vec<DiagMessage>,
}

impl Diagnostics {
    pub fn new() -> Self {
        Self {
            messages: Vec::new(),
        }
    }

    pub fn error(&mut self, span: Span, msg: impl Into<String>) {
        if self.messages.len() < 100 {
            self.messages.push(DiagMessage {
                message: msg.into(),
                span,
                is_error: true,
            });
        }
    }

    pub fn warning(&mut self, span: Span, msg: impl Into<String>) {
        if self.messages.len() < 100 {
            self.messages.push(DiagMessage {
                message: msg.into(),
                span,
                is_error: false,
            });
        }
    }

    pub fn has_errors(&self) -> bool {
        self.messages.iter().any(|m| m.is_error)
    }

    pub fn render_to_string(&self, filename: &str, source: &str) -> String {
        let mut files = SimpleFiles::new();
        let file_id = files.add(filename, source);
        let mut writer = term::termcolor::Buffer::no_color();
        let config = term::Config::default();

        for msg in &self.messages {
            let diag = if msg.is_error {
                Diagnostic::error()
            } else {
                Diagnostic::warning()
            }
            .with_message(&msg.message)
            .with_labels(vec![
                Label::primary(file_id, msg.span.start..msg.span.end).with_message(&msg.message),
            ]);

            let _ = term::emit_to_write_style(&mut writer, &config, &files, &diag);
        }

        String::from_utf8_lossy(writer.as_slice()).into_owned()
    }

    pub fn print(&self, filename: &str, source: &str) {
        let mut files = SimpleFiles::new();
        let file_id = files.add(filename, source);
        let mut writer = StandardStream::stderr(ColorChoice::Auto);
        let config = term::Config::default();

        for msg in &self.messages {
            let diag = if msg.is_error {
                Diagnostic::error()
            } else {
                Diagnostic::warning()
            }
            .with_message(&msg.message)
            .with_labels(vec![
                Label::primary(file_id, msg.span.start..msg.span.end).with_message(&msg.message),
            ]);

            let _ = term::emit_to_write_style(&mut writer, &config, &files, &diag);
        }
    }
}

impl fmt::Display for Diagnostics {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for msg in &self.messages {
            let kind = if msg.is_error { "error" } else { "warning" };
            writeln!(f, "{}: [{}] {}", kind, msg.span, msg.message)?;
        }
        Ok(())
    }
}
