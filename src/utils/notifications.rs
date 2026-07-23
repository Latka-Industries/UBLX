//! Log capture and toast data: bumper buffer, `ToastSlot` stack, `show_toast_slot`.
//!
//! - **User mode**: toast (last N messages); rendering is in [`crate::render::overlays::toast`].
//! - **Dev mode** (`--dev`): tui-logger drain + `move_log_events()`; trace-level default filter.

use std::collections::{HashMap, VecDeque};
use std::io::Write;
use std::sync::{Mutex, OnceLock, PoisonError};
use std::time::Instant;

use log::Level;
use ratatui::style::Style;

use crate::config::TOAST_CONFIG;
use crate::themes;

static BUMPER_FOR_LOG: OnceLock<BumperBuffer> = OnceLock::new();
static TUI_DRAIN: OnceLock<tui_logger::Drain> = OnceLock::new();

/// One log line for the bumper / history.
#[derive(Clone, Debug)]
pub struct BumperMessage {
    pub level: Level,
    pub text: String,
    /// Optional operation name used as the toast title (e.g. `ublx: snapshot`).
    pub operation: Option<String>,
}

/// Thread-safe ring buffer of recent log messages for the bumper.
/// Uses a [`Mutex`]; if the lock is poisoned (a thread panicked while holding it), we still acquire the guard via [`PoisonError::into_inner`] so the buffer remains usable.
#[derive(Clone)]
pub struct BumperBuffer {
    inner: std::sync::Arc<Mutex<VecDeque<BumperMessage>>>,
    cap: usize,
}

impl BumperBuffer {
    #[must_use]
    pub fn new(cap: usize) -> Self {
        Self {
            inner: std::sync::Arc::new(Mutex::new(VecDeque::with_capacity(cap))),
            cap,
        }
    }

    pub fn push(&self, level: Level, text: impl AsRef<str>) {
        self.push_with_operation(level, text.as_ref(), None::<String>);
    }

    /// Drop every message tagged with `operation` (e.g. before pushing a fresh snapshot summary so only the latest run is kept).
    pub fn remove_messages_for_operation(&self, operation: &str) {
        let mut g = self.inner.lock().unwrap_or_else(PoisonError::into_inner);
        g.retain(|m| m.operation.as_deref() != Some(operation));
    }

    /// Push a message with an operation name; the toast title uses the most recent message's operation.
    /// Text is word-wrapped to toast content width so toasts display cleanly.
    pub fn push_with_operation(
        &self,
        level: Level,
        text: &str,
        operation: Option<impl Into<String>>,
    ) {
        let content_width = TOAST_CONFIG.content_width_for(false);
        let text = wrap_text_to_width(text, content_width);
        let mut g = self.inner.lock().unwrap_or_else(PoisonError::into_inner);
        if g.len() >= self.cap {
            g.pop_front();
        }
        g.push_back(BumperMessage {
            level,
            text,
            operation: operation.map(Into::into),
        });
    }

    /// Last N messages (newest last). Returns up to `n` messages.
    pub fn last_n(&self, n: usize) -> Vec<BumperMessage> {
        let g = self.inner.lock().unwrap_or_else(PoisonError::into_inner);
        let len = g.len();
        let start = len.saturating_sub(n);
        g.range(start..).cloned().collect()
    }

    /// Last N messages that match the given operation (newest last), in chronological order.
    pub fn last_n_for_operation(&self, n: usize, operation: Option<&str>) -> Vec<BumperMessage> {
        let g = self.inner.lock().unwrap_or_else(PoisonError::into_inner);
        let mut out: Vec<BumperMessage> = g
            .iter()
            .rev()
            .filter(|m| m.operation.as_deref() == operation)
            .take(n)
            .cloned()
            .collect();
        out.reverse();
        out
    }

    /// Number of messages in the contiguous tail for this operation (from the end of the buffer).
    fn count_contiguous_for_operation(&self, operation: Option<&str>) -> usize {
        let g = self.inner.lock().unwrap_or_else(PoisonError::into_inner);
        g.iter()
            .rev()
            .take_while(|m| m.operation.as_deref() == operation)
            .count()
    }
}

/// One stacked toast: snapshot of messages and its own timer.
#[derive(Clone, Debug)]
pub struct ToastSlot {
    pub visible_until: Instant,
    pub operation: Option<String>,
    pub messages: Vec<BumperMessage>,
}

/// Word-wrap `text` so no line exceeds `max_width` chars. Inserts `\n` at word boundaries; words longer than `max_width` are broken. Existing newlines are preserved (each line is wrapped separately).
#[must_use]
pub fn wrap_text_to_width(text: &str, max_width: usize) -> String {
    if max_width == 0 {
        return text.to_string();
    }
    let mut out = String::new();
    for (i, line) in text.split('\n').enumerate() {
        if i > 0 {
            out.push('\n');
        }
        out.push_str(&wrap_single_line(line, max_width));
    }
    out
}

fn wrap_single_line(line: &str, max_width: usize) -> String {
    let line = line.trim_end();
    if line.len() <= max_width {
        return line.to_string();
    }
    let mut result = String::new();
    let mut current_len = 0usize;
    for word in line.split_whitespace() {
        let need = if current_len == 0 {
            word.len()
        } else {
            1 + word.len()
        };
        if current_len > 0 && current_len + need > max_width {
            result.push('\n');
            current_len = 0;
        }
        if current_len > 0 {
            result.push(' ');
        }
        if word.len() > max_width {
            for ch in word.chars() {
                if current_len >= max_width {
                    result.push('\n');
                    current_len = 0;
                }
                result.push(ch);
                current_len += 1;
            }
        } else {
            result.push_str(word);
            current_len += need;
        }
    }
    result
}

/// Number of content lines in a toast (one per message plus newlines within each message).
#[must_use]
pub fn toast_content_line_count(slot: &ToastSlot) -> usize {
    slot.messages
        .iter()
        .map(|m| 1 + m.text.matches('\n').count())
        .sum()
}

/// Push a new toast onto the stack. Call after pushing to bumper. Only takes messages we haven't shown yet for this operation (tracked in `consumed`).
pub fn show_toast_slot(
    slots: &mut Vec<ToastSlot>,
    bumper: &BumperBuffer,
    operation: Option<&str>,
    consumed: &mut HashMap<String, usize>,
) {
    let key = operation.unwrap_or("").to_string();
    let total = bumper.count_contiguous_for_operation(operation);
    let already = consumed.get(&key).copied().unwrap_or(0);
    let take = total.saturating_sub(already);

    // How many messages to show: new ones, or (after switching ops) current block, or fallback 1 when tail was overwritten by another op's message.
    let n = match (take > 0, total > 0) {
        (true, _) => take,
        (false, true) => total,
        (false, false) => 1,
    };
    consumed.insert(key, total);

    let messages = bumper.last_n_for_operation(n, operation);
    if messages.is_empty() {
        return;
    }

    slots.push(ToastSlot {
        visible_until: Instant::now() + TOAST_CONFIG.duration,
        operation: operation.map(String::from),
        messages,
    });
    let excess = slots.len().saturating_sub(TOAST_CONFIG.max_toast_stack);
    if excess > 0 {
        slots.drain(..excess);
    }
}

/// Write bumper contents to stderr. Call after the TUI exits so terminal is restored; safe to read in scrollback.
pub fn flush_bumper_to_stderr(bumper: &BumperBuffer) {
    let msgs = bumper.last_n(500);
    if msgs.is_empty() {
        return;
    }
    let mut out = std::io::stderr().lock();
    let _ = writeln!(out, "--- ublx log (last {} messages) ---", msgs.len());
    for m in &msgs {
        let prefix = level_short(m.level);
        let _ = writeln!(out, "{} {}", prefix, m.text);
    }
    let _ = out.flush();
}

/// Initialize logging: bumper buffer + `env_logger`. In dev, also feed `tui_logger` via its Drain.
/// Call once at startup. Pass a clone of your `BumperBuffer`; keep the original for rendering.
/// Default filter: dev = Trace, user = Warn; overridable with `RUST_LOG`.
pub fn init_logging(bumper: BumperBuffer, dev: bool) {
    let _ = BUMPER_FOR_LOG.set(bumper);
    if dev {
        let _ = TUI_DRAIN.set(tui_logger::Drain::new());
    }

    let default_filter = if dev { "trace" } else { "warn" };
    let mut builder =
        env_logger::Builder::from_env(env_logger::Env::default().default_filter_or(default_filter));

    builder
        .format(|_buf, record| {
            if let Some(b) = BUMPER_FOR_LOG.get() {
                b.push(record.level(), format!("{}", record.args()));
            }
            if let Some(d) = TUI_DRAIN.get() {
                d.log(record);
            }
            Ok(())
        })
        .init();
}

/// Call each frame in dev mode so the tui-logger widget receives new events.
pub fn move_log_events() {
    tui_logger::move_events();
}

#[must_use]
pub fn level_style(level: Level) -> Style {
    let colors = themes::DEFAULT_COLORS;
    let color = match level {
        Level::Error => colors.red,
        Level::Warn => colors.yellow,
        Level::Info => colors.cyan,
        Level::Debug => colors.magenta,
        Level::Trace => colors.gray,
    };
    Style::default().fg(color)
}

#[must_use]
pub fn level_short(level: Level) -> &'static str {
    match level {
        Level::Error => "E",
        Level::Warn => "W",
        Level::Info => "I",
        Level::Debug => "D",
        Level::Trace => "T",
    }
}

#[cfg(test)]
mod tests {
    use super::BumperBuffer;
    use log::Level;

    #[test]
    fn remove_messages_for_operation_drops_matching_only() {
        let b = BumperBuffer::new(100);
        b.push_with_operation(Level::Info, "snap", Some("ublx: snapshot"));
        b.push_with_operation(Level::Info, "lens", Some("ublx: lens"));
        b.remove_messages_for_operation("ublx: snapshot");
        let last = b.last_n(10);
        assert_eq!(last.len(), 1);
        assert_eq!(last[0].text, "lens");
    }
}
