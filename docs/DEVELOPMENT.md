# 개발 가이드

## 사전 요구사항

| 도구 | 용도 |
|---|---|
| [Rust](https://rustup.rs) (stable, `cargo`/`clippy`) | 백엔드 |
| Node 18+ & [pnpm](https://pnpm.io) | 프론트 |
| [Tauri 2 사전 요건](https://tauri.app/start/prerequisites/) (macOS: Xcode CLT) | 빌드 |
| `svn` CLI (선택) | SVN 저장소 추적/테스트 |

## 설치

```bash
pnpm install
```

> **pnpm v11 주의** — esbuild는 네이티브 바이너리 셋업에 빌드 스크립트가 필요한데 pnpm v11이 기본 차단(`ERR_PNPM_IGNORED_BUILDS`)한다. 루트 `pnpm-workspace.yaml`의 다음 설정으로 허용한다(이미 포함됨):
> ```yaml
> allowBuilds:
>   esbuild: true
> ```

## 명령

```bash
pnpm tauri dev      # 개발 실행 (Vite dev :1420 + Tauri 창, hot reload)
pnpm tauri build    # 릴리즈 .app/.dmg
pnpm dev            # Vite dev 서버만
pnpm build          # 프론트만 빌드 (→ dist/)
pnpm check          # svelte-check (타입)
pnpm test           # vitest (프론트 순수 로직)

cd src-tauri
cargo test          # Rust 단위 + 통합 테스트
cargo clippy --all-targets -- -D warnings
cargo build
```

## 검증 게이트 (CI/머지 전)

- `pnpm check` → 0 errors / 0 warnings
- `pnpm test` → 프론트 로직 테스트 통과
- `cd src-tauri && cargo test` → 단위 + 통합 통과
- `cargo clippy --all-targets -- -D warnings` → 무경고
- `pnpm build` + `cargo build` → 컴파일 성공

> GUI 시각/상호작용은 `pnpm tauri dev`로 수동 확인한다(자동화된 렌더 테스트는 없음).

## 테스트 구성

- **Rust 단위** (`#[cfg(test)]`): porcelain v2 파서·svn status 파서·config 직렬화·discovery 글롭/카테고리·clean/state 판정 등 순수 로직.
- **Rust 통합** (`src-tauri/tests/`): 임시 디렉토리에 실제 git/svn repo를 만들어(`git init`/`svnadmin create`+`svn checkout`) reader 결과를 단언.
- **프론트** (`src/lib/logic.test.ts`, vitest): clean 술어·정렬·필터·시간 포맷·그룹·요약. 컴포넌트는 `svelte-check`로 타입 검증(렌더 테스트 생략).

## 빌드/배포 메모

- `tauri.conf.json`: 단일 `main` 윈도우, `identifier = com.dgitx.repositorymonitor`, `frontendDist = ../dist`(repo 루트), `beforeBuildCommand = pnpm build`.
- 프론트 산출물 `dist/`와 `src-tauri/target`, `src-tauri/gen`, `node_modules`는 gitignore.
- 코드 서명/공증: release CI에서 Apple Developer ID로 **서명 + 공증(notarization)** 수행(시크릿 `APPLE_*` 6개 + updater `TAURI_SIGNING_PRIVATE_KEY`). 셋업 절차는 [NOTARIZATION.md](NOTARIZATION.md). 시크릿 미설정 시 미서명 빌드로 폴백.

## 코드 스타일

- Rust: `cargo fmt` 기본, `clippy --all-targets -- -D warnings` 무경고 유지.
- 프론트: TypeScript strict(`noUnusedLocals`/`noUnusedParameters`), Svelte 5 runes(`$state`/`$derived`/`$props`/`$bindable`).
- 백엔드 타입 변경 시 `src/lib/types.ts`(snake_case 미러)를 함께 갱신. git/svn 출력 파싱을 손대면 [STATUS-MAPPING.md](STATUS-MAPPING.md)를 갱신.
