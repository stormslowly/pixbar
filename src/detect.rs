use crate::Capability;
use std::env;

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
