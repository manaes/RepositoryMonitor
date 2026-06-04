use globset::{GlobSet, GlobSetBuilder};

/// 제외 글롭 집합 빌드.
/// - 매칭 대상 = repo 절대경로 전체 문자열.
/// - '/'로 시작하면 절대 패턴 그대로. 그 외는 `**/<raw>`(그 경로 자체) + `**/<raw>/**`(그 이하)
///   두 글롭을 추가해 "디렉토리 자신"과 "그 내부"를 모두 매칭.
/// - 대소문자 무시(macOS 기본 FS).
pub fn build_exclude_set(globs: &[String]) -> GlobSet {
    // 패턴 문자열을 먼저 모은다.
    let mut patterns: Vec<String> = Vec::new();
    for raw in globs {
        let raw = raw.trim().trim_end_matches('/');
        if raw.is_empty() {
            continue;
        }
        if raw.starts_with('/') {
            patterns.push(raw.to_string());
        } else {
            patterns.push(format!("**/{raw}"));
            patterns.push(format!("**/{raw}/**"));
        }
    }
    // 대소문자 무시 + literal_separator: '*'는 '/'를 넘지 않고 '**'만 넘음
    let mut builder = GlobSetBuilder::new();
    for p in &patterns {
        if let Ok(g) = globset::GlobBuilder::new(p)
            .case_insensitive(true)
            .literal_separator(true)
            .build()
        {
            builder.add(g);
        }
    }
    builder.build().unwrap_or_else(|_| GlobSet::empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bare_name_matches_anywhere() {
        // 'node_modules' → **/node_modules/** 로 확장, 절대경로 어디서나 매칭
        let set = build_exclude_set(&["node_modules".to_string()]);
        assert!(set.is_match("/Users/me/proj/node_modules/pkg"));
        assert!(!set.is_match("/Users/me/proj/src/main.rs"));
    }

    #[test]
    fn leading_slash_is_absolute() {
        let set = build_exclude_set(&["/Users/me/secret/*".to_string()]);
        assert!(set.is_match("/Users/me/secret/repo"));
        assert!(!set.is_match("/Users/other/secret/repo"));
    }

    #[test]
    fn case_insensitive() {
        let set = build_exclude_set(&["Pods".to_string()]);
        assert!(set.is_match("/a/PODS/x"));
    }

    #[test]
    fn empty_globs_match_nothing() {
        let set = build_exclude_set(&[]);
        assert!(!set.is_match("/anything/at/all"));
    }
}
