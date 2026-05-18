use crate::Capability;
use std::env;

pub fn detect() -> Capability {
    detect_from_env(|k| env::var(k).ok())
}

fn detect_from_env(get: impl Fn(&str) -> Option<String>) -> Capability {
    if let Some(v) = get("APB_FORCE_CAP") {
        return match v.as_str() {
            "ascii"     => Capability::Ascii,
            "eighth"    => Capability::EighthBlock,
            "sixteenth" => Capability::PatchedSixteenth,
            _           => Capability::EighthBlock,
        };
    }
    if get("APB_FONT_PATCHED").as_deref() == Some("1") {
        return Capability::PatchedSixteenth;
    }
    if get("WEZTERM_PANE").is_some()
        || get("GHOSTTY_RESOURCES_DIR").is_some()
        || get("KITTY_WINDOW_ID").is_some()
    {
        return Capability::EighthBlock;
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

    #[test] fn force_overrides_everything() {
        assert_eq!(
            detect_from_env(env(&[("APB_FORCE_CAP", "ascii"), ("APB_FONT_PATCHED", "1")])),
            Capability::Ascii
        );
    }
    #[test] fn patched_flag_wins_over_term_heuristic() {
        assert_eq!(
            detect_from_env(env(&[("APB_FONT_PATCHED", "1"), ("WEZTERM_PANE", "x")])),
            Capability::PatchedSixteenth
        );
    }
    #[test] fn default_is_eighth() {
        assert_eq!(detect_from_env(env(&[])), Capability::EighthBlock);
    }
    #[test] fn wezterm_keeps_eighth() {
        assert_eq!(
            detect_from_env(env(&[("WEZTERM_PANE", "x")])),
            Capability::EighthBlock
        );
    }
    #[test] fn force_eighth_string() {
        assert_eq!(
            detect_from_env(env(&[("APB_FORCE_CAP", "eighth")])),
            Capability::EighthBlock
        );
    }
    #[test] fn force_sixteenth_string() {
        assert_eq!(
            detect_from_env(env(&[("APB_FORCE_CAP", "sixteenth")])),
            Capability::PatchedSixteenth
        );
    }
    #[test] fn invalid_force_falls_back_to_eighth() {
        assert_eq!(
            detect_from_env(env(&[("APB_FORCE_CAP", "garbage")])),
            Capability::EighthBlock
        );
    }
}
