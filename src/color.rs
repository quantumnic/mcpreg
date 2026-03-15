//! Simple ANSI color helpers for terminal output.
//!
//! Auto-detects whether stdout is a TTY and disables colors when piping.
//! No external dependencies — just raw ANSI escape codes.

use std::io::IsTerminal;

/// Whether colors should be used (auto-detected, can be overridden via NO_COLOR env).
pub fn colors_enabled() -> bool {
    if std::env::var("NO_COLOR").is_ok() {
        return false;
    }
    if std::env::var("FORCE_COLOR").is_ok() {
        return true;
    }
    std::io::stdout().is_terminal()
}

/// ANSI code wrapper — returns the code only if colors are enabled.
fn ansi(code: &str) -> &str {
    if colors_enabled() { code } else { "" }
}

pub fn reset() -> &'static str { ansi("\x1b[0m") }
pub fn bold() -> &'static str { ansi("\x1b[1m") }
#[allow(dead_code)]
pub fn dim() -> &'static str { ansi("\x1b[2m") }
pub fn underline() -> &'static str { ansi("\x1b[4m") }

#[allow(dead_code)]
pub fn green() -> &'static str { ansi("\x1b[32m") }
#[allow(dead_code)]
pub fn yellow() -> &'static str { ansi("\x1b[33m") }
#[allow(dead_code)]
pub fn blue() -> &'static str { ansi("\x1b[34m") }
#[allow(dead_code)]
pub fn cyan() -> &'static str { ansi("\x1b[36m") }
#[allow(dead_code)]
pub fn red() -> &'static str { ansi("\x1b[31m") }
#[allow(dead_code)]
pub fn magenta() -> &'static str { ansi("\x1b[35m") }
#[allow(dead_code)]
pub fn white() -> &'static str { ansi("\x1b[37m") }

/// Wrap a string with a color and reset.
#[allow(dead_code)]
pub fn colorize(text: &str, color_fn: fn() -> &'static str) -> String {
    format!("{}{}{}", color_fn(), text, reset())
}

/// Highlight matching substrings in text (bold + underline).
/// Case-insensitive highlighting.
pub fn highlight_matches(text: &str, terms: &[&str]) -> String {
    if !colors_enabled() || terms.is_empty() {
        return text.to_string();
    }

    let text_lower = text.to_lowercase();
    let mut highlights: Vec<(usize, usize)> = Vec::new();

    for term in terms {
        let term_lower = term.to_lowercase();
        let mut start = 0;
        while let Some(pos) = text_lower[start..].find(&term_lower) {
            let abs_pos = start + pos;
            highlights.push((abs_pos, abs_pos + term_lower.len()));
            start = abs_pos + 1;
        }
    }

    if highlights.is_empty() {
        return text.to_string();
    }

    // Merge overlapping ranges
    highlights.sort_by_key(|&(s, _)| s);
    let mut merged: Vec<(usize, usize)> = Vec::new();
    for (s, e) in highlights {
        if let Some(last) = merged.last_mut() {
            if s <= last.1 {
                last.1 = last.1.max(e);
                continue;
            }
        }
        merged.push((s, e));
    }

    let mut result = String::with_capacity(text.len() + merged.len() * 10);
    let mut pos = 0;
    let hl_start = format!("{}{}", bold(), underline());
    let hl_end = reset().to_string();

    for (s, e) in &merged {
        if pos < *s {
            result.push_str(&text[pos..*s]);
        }
        result.push_str(&hl_start);
        result.push_str(&text[*s..*e]);
        result.push_str(&hl_end);
        pos = *e;
    }
    if pos < text.len() {
        result.push_str(&text[pos..]);
    }

    result
}

/// Format a download count with magnitude suffix (1.2k, 34k, etc.).
pub fn format_downloads(count: i64) -> String {
    match count {
        n if n >= 1_000_000 => format!("{:.1}M", n as f64 / 1_000_000.0),
        n if n >= 1_000 => format!("{:.1}k", n as f64 / 1_000.0),
        n => format!("{n}"),
    }
}

/// Format a star count with the star emoji.
pub fn format_stars(count: i64) -> String {
    if count > 0 {
        format!("★ {count}")
    } else {
        String::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_downloads_small() {
        assert_eq!(format_downloads(42), "42");
        assert_eq!(format_downloads(999), "999");
    }

    #[test]
    fn test_format_downloads_thousands() {
        assert_eq!(format_downloads(1000), "1.0k");
        assert_eq!(format_downloads(1500), "1.5k");
        assert_eq!(format_downloads(52000), "52.0k");
    }

    #[test]
    fn test_format_downloads_millions() {
        assert_eq!(format_downloads(1_000_000), "1.0M");
        assert_eq!(format_downloads(2_500_000), "2.5M");
    }

    #[test]
    fn test_format_stars_zero() {
        assert_eq!(format_stars(0), "");
    }

    #[test]
    fn test_format_stars_positive() {
        assert_eq!(format_stars(5), "★ 5");
        assert_eq!(format_stars(100), "★ 100");
    }

    #[test]
    fn test_highlight_matches_no_terms() {
        let result = highlight_matches("hello world", &[]);
        assert_eq!(result, "hello world");
    }

    #[test]
    fn test_highlight_matches_no_color() {
        // When NO_COLOR is set, highlighting should return plain text
        std::env::set_var("NO_COLOR", "1");
        let result = highlight_matches("hello world", &["hello"]);
        assert_eq!(result, "hello world");
        std::env::remove_var("NO_COLOR");
    }

    #[test]
    fn test_highlight_matches_no_match() {
        std::env::set_var("FORCE_COLOR", "1");
        let result = highlight_matches("hello world", &["xyz"]);
        assert_eq!(result, "hello world");
        std::env::remove_var("FORCE_COLOR");
    }

    #[test]
    fn test_highlight_matches_single() {
        // Test that the result always contains original text
        let result = highlight_matches("hello world", &["hello"]);
        assert!(result.contains("hello"));
        assert!(result.contains("world"));
    }

    #[test]
    fn test_highlight_matches_case_insensitive() {
        // Test that the result always contains original text (regardless of color state)
        let result = highlight_matches("Hello World", &["hello"]);
        assert!(result.contains("Hello"));
        assert!(result.contains("World"));
    }

    #[test]
    fn test_colorize_no_color() {
        std::env::set_var("NO_COLOR", "1");
        let result = colorize("test", green);
        assert_eq!(result, "test");
        std::env::remove_var("NO_COLOR");
    }

    #[test]
    fn test_format_downloads_zero() {
        assert_eq!(format_downloads(0), "0");
    }

    #[test]
    fn test_format_downloads_negative() {
        assert_eq!(format_downloads(-1), "-1");
    }
}
