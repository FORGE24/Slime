use std::io::{self, Write};
use std::sync::atomic::{AtomicBool, Ordering};

static COLOR: AtomicBool = AtomicBool::new(true);

pub fn set_color(enabled: bool) {
    COLOR.store(enabled, Ordering::Relaxed);
}

fn color_enabled() -> bool {
    COLOR.load(Ordering::Relaxed)
}

/// Byte span in the source file (half-open `[start, end)`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Span {
    pub start: u32,
    pub end: u32,
}

impl Span {
    pub fn new(start: usize, end: usize) -> Self {
        Self {
            start: start as u32,
            end: end as u32,
        }
    }

    pub fn dummy() -> Self {
        Self { start: 0, end: 0 }
    }

    pub fn merge(self, other: Span) -> Span {
        Span {
            start: self.start.min(other.start),
            end: self.end.max(other.end),
        }
    }
}

#[derive(Debug, Clone)]
pub struct SourceFile {
    pub name: String,
    pub src: String,
    /// Byte offset of the start of each line (0-indexed lines).
    line_starts: Vec<usize>,
}

impl SourceFile {
    pub fn new(name: impl Into<String>, src: impl Into<String>) -> Self {
        let src = src.into();
        let mut line_starts = vec![0];
        for (i, b) in src.bytes().enumerate() {
            if b == b'\n' {
                line_starts.push(i + 1);
            }
        }
        Self {
            name: name.into(),
            src,
            line_starts,
        }
    }

    pub fn lookup(&self, offset: usize) -> (usize, usize) {
        let offset = offset.min(self.src.len());
        let line = match self.line_starts.binary_search(&offset) {
            Ok(i) => i,
            Err(i) => i.saturating_sub(1),
        };
        let col = offset - self.line_starts[line];
        (line + 1, col + 1) // 1-based
    }

    pub fn line_text(&self, line_1based: usize) -> &str {
        let idx = line_1based.saturating_sub(1);
        let start = *self.line_starts.get(idx).unwrap_or(&0);
        let end = self
            .line_starts
            .get(idx + 1)
            .copied()
            .unwrap_or(self.src.len());
        let line = &self.src[start..end];
        line.strip_suffix('\n')
            .map(|s| s.strip_suffix('\r').unwrap_or(s))
            .unwrap_or(line)
            .strip_suffix('\r')
            .unwrap_or(line)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum Level {
    Error,
    Warning,
    Note,
    Help,
}

impl Level {
    fn label(self) -> &'static str {
        match self {
            Level::Error => "error",
            Level::Warning => "warning",
            Level::Note => "note",
            Level::Help => "help",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Label {
    pub span: Span,
    pub message: String,
}

#[derive(Debug, Clone)]
pub struct Diagnostic {
    pub level: Level,
    pub code: Option<&'static str>,
    pub message: String,
    pub labels: Vec<Label>,
    pub notes: Vec<String>,
    pub helps: Vec<String>,
}

impl Diagnostic {
    pub fn error(message: impl Into<String>) -> Self {
        Self {
            level: Level::Error,
            code: None,
            message: message.into(),
            labels: Vec::new(),
            notes: Vec::new(),
            helps: Vec::new(),
        }
    }

    pub fn code(mut self, code: &'static str) -> Self {
        self.code = Some(code);
        self
    }

    pub fn label(mut self, span: Span, message: impl Into<String>) -> Self {
        self.labels.push(Label {
            span,
            message: message.into(),
        });
        self
    }

    pub fn note(mut self, note: impl Into<String>) -> Self {
        self.notes.push(note.into());
        self
    }

    pub fn help(mut self, help: impl Into<String>) -> Self {
        self.helps.push(help.into());
        self
    }

    pub fn emit(&self, file: &SourceFile) {
        let _ = self.emit_to(&mut io::stderr(), file);
    }

    pub fn emit_to(&self, out: &mut dyn Write, file: &SourceFile) -> io::Result<()> {
        let color = color_enabled();
        let (lvl_style, reset) = match (color, self.level) {
            (true, Level::Error) => ("\x1b[1;31m", "\x1b[0m"),
            (true, Level::Warning) => ("\x1b[1;33m", "\x1b[0m"),
            (true, Level::Note | Level::Help) => ("\x1b[1;36m", "\x1b[0m"),
            _ => ("", ""),
        };
        let blue = if color { "\x1b[1;34m" } else { "" };
        let green = if color { "\x1b[1;32m" } else { "" };

        // error[E0001]: message
        write!(out, "{lvl_style}{}", self.level.label())?;
        if let Some(code) = self.code {
            write!(out, "[{code}]")?;
        }
        writeln!(out, "{reset}: {}", self.message)?;

        let primary = self.labels.first();
        if let Some(label) = primary {
            let (line, col) = file.lookup(label.span.start as usize);
            //   --> file:line:col
            writeln!(
                out,
                "{blue}  -->{reset} {}:{}:{}",
                file.name, line, col
            )?;

            let gutter = line.to_string().len().max(1);
            let pad = " ".repeat(gutter);

            writeln!(out, "{blue}{pad} |{reset}")?;

            // Group labels by line for primary display (rustc shows one line mainly)
            let line_text = file.line_text(line);
            writeln!(
                out,
                "{blue}{line:>gutter$} |{reset} {line_text}",
                line = line,
                gutter = gutter,
            )?;

            // caret underline under the span on this line
            let line_start = file.line_starts[line - 1];
            let span_start = label.span.start as usize;
            let span_end = label.span.end as usize;
            let col0 = span_start.saturating_sub(line_start);
            // If span extends past line, only underline to end of line
            let line_end_excl = file
                .line_starts
                .get(line)
                .copied()
                .unwrap_or(file.src.len());
            let end_on_line = span_end.min(line_end_excl).saturating_sub(line_start);
            let caret_len = end_on_line.saturating_sub(col0).max(1);

            // Account for tabs roughly as single width (good enough for toy)
            let spaces = " ".repeat(col0);
            let carets = "^".repeat(caret_len);
            write!(out, "{blue}{pad} |{reset} {spaces}")?;
            if label.message.is_empty() {
                writeln!(out, "{lvl_style}{carets}{reset}")?;
            } else {
                writeln!(out, "{lvl_style}{carets}{reset} {}", label.message)?;
            }

            // Additional labels on other lines
            for extra in self.labels.iter().skip(1) {
                let (el, ec) = file.lookup(extra.span.start as usize);
                let etext = file.line_text(el);
                writeln!(out, "{blue}{pad} |{reset}")?;
                writeln!(
                    out,
                    "{blue}{el:>gutter$} |{reset} {etext}",
                    el = el,
                    gutter = gutter,
                )?;
                let estart = file.line_starts[el - 1];
                let ecol0 = (extra.span.start as usize).saturating_sub(estart);
                let eline_end = file
                    .line_starts
                    .get(el)
                    .copied()
                    .unwrap_or(file.src.len());
                let eend = (extra.span.end as usize)
                    .min(eline_end)
                    .saturating_sub(estart);
                let elen = eend.saturating_sub(ecol0).max(1);
                write!(
                    out,
                    "{blue}{pad} |{reset} {spaces}{lvl_style}{carets}{reset}",
                    spaces = " ".repeat(ecol0),
                    carets = "^".repeat(elen),
                )?;
                if extra.message.is_empty() {
                    writeln!(out)?;
                } else {
                    writeln!(out, " {}", extra.message)?;
                }
                let _ = ec;
            }

            writeln!(out, "{blue}{pad} |{reset}")?;
        } else {
            // no span — still show file if we can
            writeln!(out, "{blue}  -->{reset} {}", file.name)?;
        }

        for note in &self.notes {
            writeln!(out, "{blue}   = note{reset}: {note}")?;
        }
        for help in &self.helps {
            writeln!(out, "{green}   = help{reset}: {help}")?;
        }

        Ok(())
    }
}

/// Compile-time / CLI errors without a source span.
pub fn emit_plain_error(message: &str) {
    let color = color_enabled();
    let (red, reset) = if color {
        ("\x1b[1;31m", "\x1b[0m")
    } else {
        ("", "")
    };
    eprintln!("{red}error{reset}: {message}");
}
