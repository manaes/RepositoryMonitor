use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TerminalApp {
    #[default]
    Terminal,
    Iterm,
    Custom(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)] // 누락 필드는 Default로 채움 + 알 수 없는 필드는 무시(deny_unknown_fields 미사용)
pub struct Config {
    pub version: u32,
    pub roots: Vec<String>,
    pub manual_paths: Vec<String>,
    pub exclude_globs: Vec<String>,
    pub poll_interval_secs: u32,
    pub scan_depth: u32,
    pub stale_fetch_days: u32,
    pub terminal_app: TerminalApp,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            version: 1,
            roots: Vec::new(),
            manual_paths: Vec::new(),
            exclude_globs: Vec::new(),
            poll_interval_secs: 30,
            scan_depth: 4,
            stale_fetch_days: 7,
            terminal_app: TerminalApp::Terminal,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_values() {
        let c = Config::default();
        assert_eq!(c.version, 1);
        assert_eq!(c.poll_interval_secs, 30);
        assert_eq!(c.scan_depth, 4);
        assert_eq!(c.stale_fetch_days, 7);
        assert_eq!(c.terminal_app, TerminalApp::Terminal);
        assert!(c.roots.is_empty());
    }

    #[test]
    fn partial_json_fills_defaults() {
        // 일부 필드만 있는 JSON → 누락 필드는 기본값
        let json = r#"{ "roots": ["/a"], "poll_interval_secs": 60 }"#;
        let c: Config = serde_json::from_str(json).unwrap();
        assert_eq!(c.roots, vec!["/a".to_string()]);
        assert_eq!(c.poll_interval_secs, 60);
        assert_eq!(c.scan_depth, 4);   // 기본값으로 채워짐
        assert_eq!(c.version, 1);
    }

    #[test]
    fn unknown_fields_ignored() {
        let json = r#"{ "roots": [], "future_field": 123 }"#;
        let c: Config = serde_json::from_str(json).unwrap();
        assert!(c.roots.is_empty());
    }

    #[test]
    fn terminal_app_serializes_snake_case() {
        assert_eq!(serde_json::to_string(&TerminalApp::Iterm).unwrap(), "\"iterm\"");
        let custom = TerminalApp::Custom("/Applications/Foo.app".into());
        let j = serde_json::to_value(&custom).unwrap();
        assert_eq!(j["custom"], "/Applications/Foo.app");
    }
}
