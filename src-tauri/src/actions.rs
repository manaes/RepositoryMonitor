use crate::config::TerminalApp;
use crate::model::ActionKind;

/// 플랫폼별 외부 앱 열기 명령의 (program, args)를 구성. 순수 함수라 단위테스트 가능.
pub fn open_argv(kind: &ActionKind, path: &str, terminal: &TerminalApp) -> (String, Vec<String>) {
    match kind {
        ActionKind::OpenFinder => open_file_manager_argv(path),
        ActionKind::OpenSourceTree => open_sourcetree_argv(path),
        ActionKind::OpenTerminal => open_terminal_argv(path, terminal),
    }
}

#[cfg(target_os = "macos")]
fn open_file_manager_argv(path: &str) -> (String, Vec<String>) {
    ("open".to_string(), vec![path.to_string()])
}

#[cfg(target_os = "windows")]
fn open_file_manager_argv(path: &str) -> (String, Vec<String>) {
    ("explorer".to_string(), vec![path.to_string()])
}

#[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
fn open_file_manager_argv(path: &str) -> (String, Vec<String>) {
    ("xdg-open".to_string(), vec![path.to_string()])
}

#[cfg(target_os = "macos")]
fn open_sourcetree_argv(path: &str) -> (String, Vec<String>) {
    (
        "open".to_string(),
        vec!["-a".to_string(), "SourceTree".to_string(), path.to_string()],
    )
}

#[cfg(target_os = "windows")]
fn open_sourcetree_argv(path: &str) -> (String, Vec<String>) {
    (
        "cmd".to_string(),
        vec![
            "/C".to_string(),
            "start".to_string(),
            "".to_string(),
            "SourceTree".to_string(),
            path.to_string(),
        ],
    )
}

#[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
fn open_sourcetree_argv(path: &str) -> (String, Vec<String>) {
    ("sourcetree".to_string(), vec![path.to_string()])
}

#[cfg(target_os = "macos")]
fn open_terminal_argv(path: &str, terminal: &TerminalApp) -> (String, Vec<String>) {
    let app = match terminal {
        TerminalApp::Iterm => "iTerm",
        TerminalApp::Ghostty => "Ghostty",
        TerminalApp::Custom(s) => s.as_str(),
        _ => "Terminal",
    };
    (
        "open".to_string(),
        vec!["-a".to_string(), app.to_string(), path.to_string()],
    )
}

#[cfg(target_os = "windows")]
fn open_terminal_argv(path: &str, terminal: &TerminalApp) -> (String, Vec<String>) {
    match terminal {
        TerminalApp::WindowsTerminal | TerminalApp::Terminal => {
            ("wt".to_string(), vec!["-d".to_string(), path.to_string()])
        }
        TerminalApp::Cmd => (
            "cmd".to_string(),
            vec![
                "/K".to_string(),
                "cd".to_string(),
                "/d".to_string(),
                path.to_string(),
            ],
        ),
        TerminalApp::Custom(s) => (s.clone(), vec![path.to_string()]),
        _ => (
            "powershell".to_string(),
            vec![
                "-NoExit".to_string(),
                "-Command".to_string(),
                "Set-Location -LiteralPath $args[0]".to_string(),
                path.to_string(),
            ],
        ),
    }
}

#[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
fn open_terminal_argv(path: &str, terminal: &TerminalApp) -> (String, Vec<String>) {
    match terminal {
        TerminalApp::Custom(s) => (s.clone(), vec![path.to_string()]),
        _ => (
            "x-terminal-emulator".to_string(),
            vec!["--working-directory".to_string(), path.to_string()],
        ),
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
        #[cfg(target_os = "macos")]
        assert_eq!(prog, "open");
        #[cfg(target_os = "windows")]
        assert_eq!(prog, "explorer");
        #[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
        assert_eq!(prog, "xdg-open");
        assert_eq!(args, vec!["/repo".to_string()]);
    }

    #[test]
    fn sourcetree_argv() {
        let (prog, args) = open_argv(&ActionKind::OpenSourceTree, "/repo", &TerminalApp::Terminal);
        #[cfg(target_os = "macos")]
        {
            assert_eq!(prog, "open");
            assert_eq!(
                args,
                vec![
                    "-a".to_string(),
                    "SourceTree".to_string(),
                    "/repo".to_string()
                ]
            );
        }
        #[cfg(target_os = "windows")]
        {
            assert_eq!(prog, "cmd");
            assert_eq!(args, vec!["/C", "start", "", "SourceTree", "/repo"]);
        }
    }

    #[test]
    fn terminal_argv_variants() {
        #[cfg(target_os = "macos")]
        {
            let (_, a1) = open_argv(&ActionKind::OpenTerminal, "/r", &TerminalApp::Terminal);
            assert_eq!(
                a1,
                vec!["-a".to_string(), "Terminal".to_string(), "/r".to_string()]
            );
            let (_, a2) = open_argv(&ActionKind::OpenTerminal, "/r", &TerminalApp::Iterm);
            assert_eq!(
                a2,
                vec!["-a".to_string(), "iTerm".to_string(), "/r".to_string()]
            );
            let (_, a3) = open_argv(&ActionKind::OpenTerminal, "/r", &TerminalApp::Ghostty);
            assert_eq!(
                a3,
                vec!["-a".to_string(), "Ghostty".to_string(), "/r".to_string()]
            );
            let (_, a4) = open_argv(
                &ActionKind::OpenTerminal,
                "/r",
                &TerminalApp::Custom("/Applications/Foo.app".into()),
            );
            assert_eq!(
                a4,
                vec![
                    "-a".to_string(),
                    "/Applications/Foo.app".to_string(),
                    "/r".to_string()
                ]
            );
        }
        #[cfg(target_os = "windows")]
        {
            let (p1, a1) = open_argv(
                &ActionKind::OpenTerminal,
                "C:\\repo",
                &TerminalApp::WindowsTerminal,
            );
            assert_eq!(p1, "wt");
            assert_eq!(a1, vec!["-d".to_string(), "C:\\repo".to_string()]);
            let (p2, a2) = open_argv(
                &ActionKind::OpenTerminal,
                "C:\\repo",
                &TerminalApp::Powershell,
            );
            assert_eq!(p2, "powershell");
            assert_eq!(
                a2,
                vec![
                    "-NoExit",
                    "-Command",
                    "Set-Location -LiteralPath $args[0]",
                    "C:\\repo"
                ]
            );
            let (p3, a3) = open_argv(&ActionKind::OpenTerminal, "C:\\repo", &TerminalApp::Cmd);
            assert_eq!(p3, "cmd");
            assert_eq!(a3, vec!["/K", "cd", "/d", "C:\\repo"]);
        }
    }
}
