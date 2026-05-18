//! Environment-driven capability auto-detection.
//!
//! Priority chain:
//! 1. `APB_FORCE_CAP=ascii|eighth` — explicit override.
//! 2. Default → [`Capability::EighthBlock`].
//!
//! `Ascii` is never auto-selected; users in Unicode-hostile environments
//! must opt in via the env var. Detection performs no terminal capability
//! probing — it reads env vars only.

use crate::Capability;
use std::env;

/// Detect the best capability for the current process environment.
pub fn detect() -> Capability {
    detect_from_env(|k| env::var(k).ok())
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
}
