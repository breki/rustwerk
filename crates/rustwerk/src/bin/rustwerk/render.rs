//! Output rendering for CLI commands.
//!
//! Every `cmd_*` function returns an owned DTO that
//! implements both `serde::Serialize` (for `--json`) and
//! the `RenderText` trait (for the default human
//! output). This module holds the trait, the dispatch
//! helper, and the JSON printer. Keeping presentation
//! logic out of the `cmd_*` bodies means adding a new
//! output format (yaml, ndjson, ...) is a single-site
//! change.

use std::io::{self, Write};

use anyhow::Result;
use serde::Serialize;

/// Render a command result as human-readable text.
pub(crate) trait RenderText {
    /// Write the text representation to `w`.
    fn render_text(&self, w: &mut dyn Write) -> io::Result<()>;
}

/// Selected output format.
#[derive(Debug, Clone, Copy)]
pub(crate) enum OutputFormat {
    /// Human-readable text (the default).
    Text,
    /// Pretty-printed JSON DTO.
    Json,
}

impl OutputFormat {
    /// Pick format based on whether the global `--json`
    /// flag was set.
    pub(crate) fn from_json_flag(json: bool) -> Self {
        if json {
            Self::Json
        } else {
            Self::Text
        }
    }
}

/// Replace non-finite `f64` values with `None` so the
/// JSON output stays valid — `serde_json` refuses to
/// serialize `NaN` / `Infinity`.
pub(crate) fn finite(value: f64) -> Option<f64> {
    if value.is_finite() {
        Some(value)
    } else {
        None
    }
}

/// Emit a command result in the chosen format.
pub(crate) fn emit<T>(result: &T, format: OutputFormat) -> Result<()>
where
    T: Serialize + RenderText,
{
    let stdout = io::stdout();
    let mut handle = stdout.lock();
    let io_result = match format {
        OutputFormat::Json => write_json(&mut handle, result),
        OutputFormat::Text => result.render_text(&mut handle),
    };
    // Broken pipe is normal when piping to `head` or
    // closing the consumer; exit cleanly.
    match io_result {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == io::ErrorKind::BrokenPipe => Ok(()),
        Err(e) => Err(anyhow::Error::from(e).context("failed to write output")),
    }
}

fn write_json<T: Serialize, W: Write>(
    w: &mut W,
    value: &T,
) -> io::Result<()> {
    serde_json::to_writer_pretty(&mut *w, value).map_err(io::Error::other)?;
    w.write_all(b"\n")
}

