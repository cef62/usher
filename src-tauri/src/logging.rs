use log::{Level, LevelFilter};
use std::str::FromStr;
use tauri_plugin_log::{RotationStrategy, Target, TargetKind, TimezoneStrategy};

/// Targets that get debug detail; everything else (tao, wry, tauri...) stays at info. The webview target is
/// `webview` or `webview::<location>` (see the plugin), and a single colon is accepted too.
const USHER_TARGETS: &[&str] = &[
    "usher",
    "usher_lib",
    "usher_core",
    "usher_coreaudio",
    tauri_plugin_log::WEBVIEW_TARGET,
];

/// `USHER_LOG` (trace..error, any case) wins; else debug for a debug build or the Debug setting; else info.
pub fn log_level(env: Option<&str>, debug_build: bool, debug_setting: bool) -> LevelFilter {
    env.and_then(|s| LevelFilter::from_str(s.trim()).ok())
        .unwrap_or(if debug_build || debug_setting {
            LevelFilter::Debug
        } else {
            LevelFilter::Info
        })
}

pub fn allowed(target: &str, level: Level, max: LevelFilter) -> bool {
    let ours = USHER_TARGETS.iter().any(|t| {
        target
            .strip_prefix(t)
            .is_some_and(|rest| rest.is_empty() || rest.starts_with(':'))
    });
    level <= max && (ours || level <= Level::Info)
}

pub fn plugin<R: tauri::Runtime>() -> tauri::plugin::TauriPlugin<R> {
    tauri_plugin_log::Builder::new()
        .clear_targets()
        .targets([
            Target::new(TargetKind::Stdout),
            Target::new(TargetKind::LogDir {
                file_name: Some("usher".into()),
            }),
        ])
        .max_file_size(1_000_000)
        // KeepSome counts rotated files only: 4 + the active one = the spec's 5 files
        .rotation_strategy(RotationStrategy::KeepSome(4))
        .timezone_strategy(TimezoneStrategy::UseLocal)
        // the level is left at Trace so log::set_max_level alone moves it at runtime
        .filter(|m| allowed(m.target(), m.level(), log::max_level()))
        .build()
}

/// Sets the runtime log level (M4 calls it again when the Debug toggle changes) and logs it.
pub fn apply(debug_setting: bool) -> LevelFilter {
    let level = log_level(
        std::env::var("USHER_LOG").ok().as_deref(),
        cfg!(debug_assertions),
        debug_setting,
    );
    log::set_max_level(level);
    log::info!("log level {level}");
    level
}

#[cfg(test)]
mod tests {
    use super::*;
    use log::{Level, LevelFilter as F};
    #[test]
    fn level_defaults_to_info_in_release_without_setting() {
        assert_eq!(log_level(None, false, false), F::Info);
    }
    #[test]
    fn debug_build_or_setting_turns_debug_on() {
        assert_eq!(log_level(None, true, false), F::Debug);
        assert_eq!(log_level(None, false, true), F::Debug);
    }
    #[test]
    fn usher_log_wins_and_is_parsed_loosely() {
        assert_eq!(log_level(Some("debug"), false, false), F::Debug);
        assert_eq!(log_level(Some(" DEBUG "), false, false), F::Debug);
        assert_eq!(log_level(Some("trace"), false, false), F::Trace);
        assert_eq!(log_level(Some("warn"), true, true), F::Warn);
    }
    #[test]
    fn unknown_or_empty_usher_log_falls_back() {
        assert_eq!(log_level(Some(""), false, false), F::Info);
        assert_eq!(log_level(Some("verbose"), true, false), F::Debug);
    }
    #[test]
    fn debug_detail_is_for_usher_crates_only() {
        for t in [
            "usher",
            "usher_lib::tray",
            "usher_core",
            "usher_coreaudio::sys",
            "webview:http://localhost",
        ] {
            assert!(allowed(t, Level::Debug, F::Debug), "{t}");
        }
        for t in ["tao::platform", "wry", "usher_libx", "usherx", "tauri"] {
            assert!(!allowed(t, Level::Debug, F::Debug), "{t}");
            assert!(allowed(t, Level::Info, F::Debug), "{t}");
        }
    }
    #[test]
    fn max_level_caps_everyone() {
        assert!(!allowed("usher_lib", Level::Debug, F::Info));
        assert!(!allowed("tao", Level::Info, F::Warn));
        assert!(allowed("usher_lib", Level::Error, F::Error));
    }
}
