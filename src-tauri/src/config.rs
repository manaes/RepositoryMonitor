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

use std::path::{Path, PathBuf};

/// 설정 파일 경로: 플랫폼 config 디렉토리 하위 RepositoryMonitor/config.json
pub fn config_path() -> PathBuf {
    dirs_next::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("RepositoryMonitor")
        .join("config.json")
}

/// 파일에서 Config 로드. 없으면 기본값. 파싱 실패 시 .bak 백업 후 기본값 재생성.
pub fn load_from(path: &Path) -> Config {
    let text = match std::fs::read_to_string(path) {
        Ok(t) => t,
        Err(_) => return Config::default(), // 파일 없음/읽기 실패 → 기본값
    };
    match serde_json::from_str::<Config>(&text) {
        Ok(c) => c,
        Err(_) => {
            // 손상 파일 백업 후 기본값
            let bak = path.with_extension("json.bak");
            let _ = std::fs::rename(path, &bak);
            Config::default()
        }
    }
}

/// Config를 파일에 저장(부모 디렉토리 자동 생성, pretty JSON).
pub fn save_to(path: &Path, cfg: &Config) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let text = serde_json::to_string_pretty(cfg)
        .map_err(std::io::Error::other)?;
    std::fs::write(path, text)
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

    #[test]
    fn save_then_load_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.json");
        let c = Config {
            roots: vec!["/x".into()],
            poll_interval_secs: 45,
            ..Default::default()
        };
        save_to(&path, &c).unwrap();
        let loaded = load_from(&path);
        assert_eq!(loaded, c);
    }

    #[test]
    fn missing_file_returns_default() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nope.json");
        assert_eq!(load_from(&path), Config::default());
    }

    #[test]
    fn corrupt_file_backs_up_and_returns_default() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.json");
        std::fs::write(&path, b"{ this is not json").unwrap();
        let loaded = load_from(&path);
        assert_eq!(loaded, Config::default());
        // 손상 파일은 .bak로 백업됨
        assert!(path.with_extension("json.bak").exists());
    }

    #[test]
    fn config_path_ends_with_expected() {
        let p = config_path();
        assert!(p.ends_with("RepositoryMonitor/config.json"));
    }
}
