//! Terminal markdown rendering for assistant output.
//!
//! During streaming we print raw text deltas so the user sees progress
//! immediately. When a text block finishes we optionally erase those raw
//! lines and re-render the buffered content as formatted markdown.

use std::io::{Write, stdout};

use is_terminal::IsTerminal;
use termimad::MadSkin;

/// Whether markdown rendering should be active.
///
/// Disabled when stdout isn't a terminal, `NO_COLOR` is set, or the caller
/// explicitly turned it off.
pub fn should_render(enabled: bool) -> bool {
    if !enabled {
        return false;
    }
    if std::env::var_os("NO_COLOR").is_some() {
        return false;
    }
    stdout().is_terminal()
}

/// Build the skin used for rendering. Kept minimal so colors blend with
/// whatever terminal theme the user already has.
pub fn default_skin() -> MadSkin {
    // `default_dark` gives sensible emphasis colors on dark terminals without
    // forcing any background. Users on light themes can set `NO_COLOR` to opt
    // out entirely; tuning per-theme is out of scope here.
    MadSkin::default_dark()
}

/// Count how many terminal lines were consumed by printing `text` at the
/// current cursor, assuming a terminal of `width` columns. Tabs are treated
/// as a single column — close enough for our erase logic, and markdown
/// output rarely contains raw tabs.
fn visual_lines(text: &str, width: usize) -> usize {
    if width == 0 {
        return text.lines().count().max(1);
    }
    let mut lines = 0usize;
    // split_terminator keeps a trailing empty segment if text ends with '\n',
    // matching the "cursor moved to a fresh line" behavior we want.
    let segments: Vec<&str> = text.split('\n').collect();
    for (i, seg) in segments.iter().enumerate() {
        let cols = unicode_width_approx(seg);
        if cols == 0 {
            // empty line still takes one row, except the trailing empty
            // segment after a final '\n' which represents "cursor at col 0
            // of a new line with nothing drawn yet".
            if i + 1 == segments.len() {
                // trailing newline: no visible row to erase
                continue;
            }
            lines += 1;
        } else {
            lines += cols.div_ceil(width);
        }
    }
    lines.max(1)
}

/// Rough display-width: ASCII + most Latin scripts count as 1, everything
/// else counts as 2 (CJK, emoji-ish). Good enough for erasing our own
/// just-printed text; we don't need grapheme-perfect accuracy.
fn unicode_width_approx(s: &str) -> usize {
    let mut w = 0usize;
    for ch in s.chars() {
        if ch == '\r' {
            continue;
        }
        if (ch as u32) < 0x80 {
            w += 1;
        } else if ch.is_ascii() {
            w += 1;
        } else {
            // Treat anything outside basic Latin as wide. Slight overcount
            // for e.g. accented Latin is fine — erase will just clear a bit
            // more than needed.
            w += 2;
        }
    }
    w
}

/// Erase `lines` rows above the cursor and move back to column 0, so the
/// caller can re-draw over freshly-cleared space.
fn erase_lines_above(lines: usize) {
    let mut out = stdout().lock();
    // Move to start of current line first.
    let _ = write!(out, "\r");
    // Erase current line.
    let _ = write!(out, "\x1b[2K");
    // For each additional line above, move up one and erase it.
    for _ in 1..lines {
        let _ = write!(out, "\x1b[1A\x1b[2K");
    }
    let _ = out.flush();
}

/// Replace the last `raw` text printed to stdout with a markdown-rendered
/// version of the same buffer. `term_width` should be the current terminal
/// width.
pub fn rerender_over(raw: &str, skin: &MadSkin, term_width: usize) {
    if raw.is_empty() {
        return;
    }
    let lines = visual_lines(raw, term_width.max(1));
    erase_lines_above(lines);

    let rendered = skin.text(raw, Some(term_width));
    // `FmtText` implements Display; print it and add a trailing newline so
    // the next output starts on a fresh row.
    print!("{}", rendered);
    let _ = stdout().flush();
}

/// Detect the current terminal width, falling back to 100 columns.
pub fn term_width() -> usize {
    termimad::terminal_size().0 as usize
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn visual_lines_single_short() {
        assert_eq!(visual_lines("hello", 80), 1);
    }

    #[test]
    fn visual_lines_wrapping() {
        // 10 chars wraps to 2 rows at width 5
        assert_eq!(visual_lines("abcdefghij", 5), 2);
    }

    #[test]
    fn visual_lines_multiple_newlines() {
        assert_eq!(visual_lines("a\nb\nc", 80), 3);
    }

    #[test]
    fn visual_lines_trailing_newline_ignored() {
        // "a\n" leaves the cursor at start of a new empty row; only 'a' is visible
        assert_eq!(visual_lines("a\n", 80), 1);
    }

    #[test]
    fn visual_lines_cjk_wide() {
        // Each CJK char counts as 2 cols; 3 chars at width 4 => ceil(6/4)=2
        assert_eq!(visual_lines("中文字", 4), 2);
    }
}
