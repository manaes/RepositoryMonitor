/// 폴링 tick에서 상태 배치를 돌려야 하는가: 창이 포커스 상태이고 진행 중 배치가 없을 때만.
pub fn should_run_poll(polling_active: bool, in_flight: bool) -> bool {
    polling_active && !in_flight
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn polls_only_when_active_and_idle() {
        assert!(should_run_poll(true, false));   // 포커스 O, in-flight X → 폴링
        assert!(!should_run_poll(false, false));  // 비포커스 → 스킵
        assert!(!should_run_poll(true, true));    // 이미 진행 중 → 스킵
        assert!(!should_run_poll(false, true));
    }
}
