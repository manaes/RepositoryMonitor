# macOS 코드 서명 + 공증 (Notarization)

미서명 앱은 다운로드 시 "손상되었기 때문에 열 수 없습니다"(Gatekeeper 격리)가 뜨고, 사용자가 `xattr -cr`를 직접 실행해야 한다. **Apple Developer ID로 서명 + 공증**하면 사용자가 아무 조치 없이 바로 실행할 수 있고, 자동 업데이트(updater) 재실행도 매끄러워진다.

CI(`.github/workflows/release.yml`)는 이미 `APPLE_*` env를 받도록 배선돼 있다. **아래 6개 GitHub 시크릿만 채우면** 다음 릴리즈부터 tauri가 자동으로 서명·공증한다. (시크릿이 비어 있으면 그냥 미서명으로 빌드 — 현재 동작.)

> **전제**: [Apple Developer Program](https://developer.apple.com/programs/) 가입($99/년). DG-ITX에 기존 계정이 있으면 그 팀을 사용.

---

## 1. Developer ID Application 인증서 발급

배포(App Store 밖)용은 **"Developer ID Application"** 인증서가 필요하다(개발용 "Apple Development"가 아님).

가장 쉬운 방법(Xcode):
1. Xcode → Settings → Accounts → Apple ID 로그인 → 팀 선택 → **Manage Certificates**.
2. 좌하단 **+** → **Developer ID Application** 생성.

또는 웹:
1. https://developer.apple.com/account/resources/certificates → **+** → **Developer ID Application** → CSR 업로드(키체인 접근 → 인증서 지원 → 인증 기관에서 인증서 요청)로 발급 → 다운로드 → 더블클릭해 키체인에 설치.

## 2. .p12로 내보내기 (개인키 포함)

1. **키체인 접근(Keychain Access)** → "로그인" 키체인 → 분류: 내 인증서.
2. **"Developer ID Application: <이름> (<TEAMID>)"** 항목을 펼쳐 **인증서 + 개인키**를 함께 선택 → 우클릭 → **2개 항목 내보내기** → `.p12` 저장 → **내보내기 암호** 설정(기억해둘 것).

## 3. 값 6개 준비

```bash
# (a) 인증서를 base64로 (→ APPLE_CERTIFICATE)
base64 -i Certificates.p12 | pbcopy   # 클립보드에 복사됨

# (b) 서명 ID 문자열 확인 (→ APPLE_SIGNING_IDENTITY), 예: "Developer ID Application: DG-ITX (ABCDE12345)"
security find-identity -v -p codesigning

# (c) Team ID (→ APPLE_TEAM_ID): 위 괄호 안 10자리, 또는 developer.apple.com → Membership
```

- **(d) APPLE_CERTIFICATE_PASSWORD**: 2단계에서 설정한 .p12 내보내기 암호.
- **(e) APPLE_ID**: Apple Developer 계정 이메일.
- **(f) APPLE_PASSWORD**: **앱 암호(app-specific password)** — 실제 Apple ID 비번 아님. https://account.apple.com → 로그인 및 보안 → **앱 암호** 생성.

## 4. GitHub 시크릿 등록

Repo → Settings → Secrets and variables → **Actions** → New repository secret. 아래 6개:

| 시크릿 | 값 |
|---|---|
| `APPLE_CERTIFICATE` | `.p12`의 base64 (3-a) |
| `APPLE_CERTIFICATE_PASSWORD` | `.p12` 내보내기 암호 (3-d) |
| `APPLE_SIGNING_IDENTITY` | `Developer ID Application: ... (TEAMID)` (3-b) |
| `APPLE_ID` | Apple Developer 이메일 (3-e) |
| `APPLE_PASSWORD` | 앱 암호 (3-f) |
| `APPLE_TEAM_ID` | 10자리 팀 ID (3-c) |

> ⚠️ **반드시 repository "Actions" 시크릿**에 넣을 것(Environment/다른 탭 아님). 안 그러면 워크플로우가 못 읽어서 빈 값 → 미서명으로 빌드된다. (updater 키 `TAURI_SIGNING_PRIVATE_KEY`와 같은 위치.)

## 5. 릴리즈

평소대로 버전 올리고 태그 push:
```bash
# 버전 1.0.5로 올린 뒤
git tag -a v1.0.5 -m "..." && git push origin v1.0.5
```
mac 빌드 잡에서 tauri가 `APPLE_*`를 감지해 **hardened runtime으로 서명 → Apple에 공증 제출 → staple**까지 자동 수행한다. 완료되면 .dmg/.app가 공증된 상태로 Release에 올라가고, 사용자는 `xattr` 없이 바로 실행 가능.

> 공증 제출은 Apple 서버 처리라 빌드 시간이 수 분 더 걸린다. 첫 시도에서 인증서/identity 문자열 불일치가 흔하니, 실패 시 Actions 로그의 `codesign`/`notarytool` 메시지를 확인.

---

## 참고: App Store Connect API 키 방식(대안)

`APPLE_ID`/`APPLE_PASSWORD`(앱 암호) 대신 API 키를 쓰려면 다음 3개로 대체:
`APPLE_API_ISSUER`, `APPLE_API_KEY`(Key ID), `APPLE_API_KEY_PATH`(.p8 경로). CI에서는 .p8를 파일로 떨군 뒤 경로를 지정해야 해 다소 번거로우니, 위 Apple ID + 앱 암호 방식을 권장.

## 참고: Windows 코드 서명(별개)

Windows의 "알 수 없는 게시자(SmartScreen)" 경고를 없애려면 별도의 코드 서명 인증서(OV/EV)가 필요하다(연간 비용 별도). macOS 공증과는 무관하며, 필요 시 별도로 다룬다.
