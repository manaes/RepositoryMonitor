import { check } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";
import { ask } from "@tauri-apps/plugin-dialog";

/**
 * 시작 시 1회 업데이트 확인 → 새 버전이 있으면 사용자 확인 후 다운로드·설치·재시작.
 * 개발 모드/오프라인/릴리즈 없음 등으로 실패하면 조용히 무시한다.
 */
export async function checkForUpdate(): Promise<void> {
  try {
    const update = await check();
    if (!update) return;

    const body = (update.body ?? "").trim();
    const message = `새 버전 ${update.version}이(가) 있습니다. 지금 설치할까요?${body ? `\n\n${body}` : ""}`;
    const yes = await ask(message, {
      title: "RepositoryMonitor 업데이트",
      kind: "info",
      okLabel: "설치 후 재시작",
      cancelLabel: "나중에",
    });
    if (!yes) return;

    await update.downloadAndInstall();
    await relaunch();
  } catch (e) {
    console.error("업데이트 확인 실패:", e);
  }
}
