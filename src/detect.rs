//! Environment-driven capability and color auto-detection.
//!
//! Two orthogonal detections live here:
//!
//! - [`detect`] picks a [`Capability`] (glyph set). Priority:
//!   1. `APB_FORCE_CAP=ascii|eighth` — explicit override.
//!   2. Default → [`Capability::EighthBlock`].
//!
//!   `Ascii` is never auto-selected; opt in via the env var if your
//!   environment is Unicode-hostile.
//!
//! - [`detect_color`] decides whether to emit ANSI color escapes.
//!   Priority:
//!   1. `NO_COLOR` set (any value, per <https://no-color.org/>) → `false`.
//!   2. `stdout` is not a TTY → `false`.
//!   3. Otherwise → `true`.
//!
//! Capability and color are intentionally separate: UTF-8 block glyphs
//! and 24-bit color are unrelated terminal capabilities, and a user who
//! sets `NO_COLOR` for a pipe still wants pretty 1/8-block glyphs in the
//! log file.

use crate::Capability;
use std::env;
use std::io::IsTerminal;

/// Detect the best [`Capability`] tier for the current process environment.
pub fn detect() -> Capability {
    detect_from_env(|k| env::var(k).ok())
}

/// Detect whether ANSI color should be emitted by default.
///
/// Returns `false` if `NO_COLOR` is set (any value) or if `stdout` is
/// not a TTY; otherwise `true`. Use this with [`crate::Bar::color`] or
/// [`crate::Bar::auto_color`] to wire auto-detection into the builder.
pub fn detect_color() -> bool {
    detect_color_from(|k| env::var(k).ok(), std::io::stdout().is_terminal())
}

fn detect_from_env(get: impl Fn(&str) -> Option<String>) -> Capability {
    if let Some(v) = get("APB_FORCE_CAP") {
        return match v.as_str() {
            "ascii"  => Capability::Ascii,
            "eighth" => Capability::EighthBlock,
            _        => Capability::EighthBlock,
        };
    }
    Capability::EighthBlock
}

fn detect_color_from(get: impl Fn(&str) -> Option<String>, is_tty: bool) -> bool {
    if get("NO_COLOR").is_some() {
        return false;
    }
    is_tty
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn env(pairs: &[(&'static str, &'static str)]) -> impl Fn(&str) -> Option<String> {
        let map: HashMap<&'static str, &'static str> = pairs.iter().copied().collect();
        move |k| map.get(k).map(|s| s.to_string())
    }

    #[test] fn force_overrides_default() {
        assert_eq!(
            detect_from_env(env(&[("APB_FORCE_CAP", "ascii")])),
            Capability::Ascii
        );
    }
    #[test] fn default_is_eighth() {
        assert_eq!(detect_from_env(env(&[])), Capability::EighthBlock);
    }
    #[test] fn force_eighth_string() {
        assert_eq!(
            detect_from_env(env(&[("APB_FORCE_CAP", "eighth")])),
            Capability::EighthBlock
        );
    }
    #[test] fn invalid_force_falls_back_to_eighth() {
        assert_eq!(
            detect_from_env(env(&[("APB_FORCE_CAP", "garbage")])),
            Capability::EighthBlock
        );
    }

    #[test] fn color_off_when_no_color_set() {
        assert!(!detect_color_from(env(&[("NO_COLOR", "1")]), true));
        assert!(!detect_color_from(env(&[("NO_COLOR", "")]),  true));
    }
    #[test] fn color_off_when_not_tty() {
        assert!(!detect_color_from(env(&[]), false));
    }
    #[test] fn color_on_when_tty_and_no_no_color() {
        assert!(detect_color_from(env(&[]), true));
    }
    #[test] fn no_color_beats_tty() {
        assert!(!detect_color_from(env(&[("NO_COLOR", "1")]), true));
    }
}
