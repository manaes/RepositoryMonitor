use crate::config::TerminalApp;
use crate::model::ActionKind;

/// macOS `open` 명령의 (program, args)를 구성. 순수 함수라 단위테스트 가능.
pub fn open_argv(kind: &ActionKind, path: &str, terminal: &TerminalApp) -> (String, Vec<String>) {
    match kind {
        ActionKind::OpenFinder => ("open".to_string(), vec![path.to_string()]),
        ActionKind::OpenSourceTree => (
            "open".to_string(),
            vec!["-a".to_string(), "SourceTree".to_string(), path.to_string()],
        ),
        ActionKind::OpenTerminal => {
            let app = match terminal {
                TerminalApp::Terminal => "Terminal",
                TerminalApp::Iterm => "iTerm",
                TerminalApp::Custom(s) => s.as_str(),
            };
            (
                "open".to_string(),
                vec!["-a".to_string(), app.to_string(), path.to_string()],
            )
        }
    }
}

/// 실제 외부 앱 실행. 실패(앱 미설치 등)는 Err(String).
pub fn run_action(kind: &ActionKind, path: &str, terminal: &TerminalApp) -> Result<(), String> {
    let (prog, args) = open_argv(kind, path, terminal);
    std::process::Command::new(prog)
        .args(args)
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("외부 앱 실행 실패: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::TerminalApp;
    use crate::model::ActionKind;

    #[test]
    fn finder_argv() {
        let (prog, args) = open_argv(&ActionKind::OpenFinder, "/repo", &TerminalApp::Terminal);
        assert_eq!(prog, "open");
        assert_eq!(args, vec!["/repo".to_string()]);
    }

    #[test]
    fn sourcetree_argv() {
        let (prog, args) = open_argv(&ActionKind::OpenSourceTree, "/repo", &TerminalApp::Terminal);
        assert_eq!(prog, "open");
        assert_eq!(args, vec!["-a".to_string(), "SourceTree".to_string(), "/repo".to_string()]);
    }

    #[test]
    fn terminal_argv_variants() {
        let (_, a1) = open_argv(&ActionKind::OpenTerminal, "/r", &TerminalApp::Terminal);
        assert_eq!(a1, vec!["-a".to_string(), "Terminal".to_string(), "/r".to_string()]);
        let (_, a2) = open_argv(&ActionKind::OpenTerminal, "/r", &TerminalApp::Iterm);
        assert_eq!(a2, vec!["-a".to_string(), "iTerm".to_string(), "/r".to_string()]);
        let (_, a3) = open_argv(&ActionKind::OpenTerminal, "/r", &TerminalApp::Custom("/Applications/Foo.app".into()));
        assert_eq!(a3, vec!["-a".to_string(), "/Applications/Foo.app".to_string(), "/r".to_string()]);
    }
}
