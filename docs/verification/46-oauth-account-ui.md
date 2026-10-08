# #46 OAuth 로그인·최소 계정 화면 검증

FR01/10/11/16, NFR01/02/06 · TS01/19/26/31/34/35/36 · M3 · 선행 #45 PR49 병합/종료 · [PR50](https://github.com/crystal23733/search-mine/pull/50). [ADR0022](../adr/0022-web-oauth-account-ui.md), [HTTP](../design/06-protocol.md), [화면 기준](../design/18-design-layout.md)을 따른다. 수치는 [auth-ui-summary.json](auth-ui-summary.json)에 기록한다.

## 구현한 동작

Google·Apple·Kakao·Naver와 새 제공자를 확장할 공통 catalog/port, 로그인·첫 닉네임·계정 설정을 연결했다. 미설정은 disabled와 안내, 장애는 재확인과 무계정 연습을 제공한다. 비밀번호·이메일·실명·사진·전화 입력이 없으며 요청은 선택한 제공자와 locale/허용 목적지만 사용한다. Atomic ProviderButton/NicknameField와 NicknameForm/AccountSettings는 공통 토큰과8언어를 사용한다.

AuthPort는 CSRF/account/session UUID를 메모리에만 둔다. HTTP는 same-origin/no-store, 고정 제공자 HTTPS origin/path, strict DTO,64KiB 스트림/10초 deadline을 적용한다. 정상400 입력 오류와 신뢰할 수 없는 응답 오류를 구분하고 후자는 온라인 권위를 철회한다. mutation 전에 bootstrap으로 최신 CSRF와 account/session을 대조한다. read DTO에는 CSRF/credential을 넣지 않는다. BroadcastChannel은 invalidate 문자열만 전달하고 offline/online/visibility/BFCache 때 권위를 갱신한다.

닉네임 PATCH는 실제 서버의 NFC/Unicode 응답을 적용한다. JS 문자수 표시는 편의 안내이며 Rust 규칙을 복제하지 않는다. 연결/해제·최소 JSON 다운로드·명시 재인증·서비스 로그아웃·삭제 확인을 제공한다. 마지막 제공자 해제는 disabled, 확인은 열 때 account UUID에 묶이고 닫기에 최초 focus를 둔다. 재인증 뒤 삭제를 자동 실행하지 않는다. Apple 외부 철회의 성공을 보장하지 않고 직접 철회/최대24시간 재시도 정보를 표시한다.

현재 인증 account/revision/connectivity를 PendingSubmissions에 연결했다. 공식 API #19가 없으므로 전송 port는 unavailable이며 unsupported 성공으로 대기를 지우지 않는다. 로그인·로그아웃·장애·늦은 acknowledgement에서도 원래 로컬 대기를 유지한다. 계정 삭제와 로컬 연습 자료 삭제는 별도다.

## 실제 실행과 리뷰

- 제품 source7c0eb143e227c39ff864afc27ccce7c79c3964d8의 scripts/check.ps1 전체 exit0: Rust/native/WASM·타입/fixture/tokens·format/lint/type·Vitest89·build·Chromium PC/mobile/offline50(57.9s)·docs100/Mermaid34. 로컬 DB16개 ignored, DATABASE_URL 없는 HTTPS auth project는 실행하지 않았다.
- 후속 source22c4c6762df594775b2e8393cc8ac767f9a4e4b2에서 텍스트 저장 중 손상된 번역3키와 문서/리뷰를 UTF-8로 복구했다. 번역 무결성 시험의 실제 Red→Green과 format/lint/type/Vitest90/build/docs101/Mermaid34를 다시 실행했다. 앞선 전체 check와 후속 검사를 구분한다.
- 실제 [PostgreSQL18·HTTPS browser CI](https://github.com/crystal23733/search-mine/actions/runs/37779475034/job/113319048432) source68df1b2d6fb523ff6891821fac69e06ed14b4b32는 전체57 passed(3.7m), flaky/skip 없음. 제품 router/service/PgAuthStore와 실DB를 사용하고 외부 제공자 proof만 시험 port로 대체한다.
- 네 제공자×PC/mobile8 로그인·보안/HttpOnly/Lax cookie·Unicode nickname·export/link/unlink/최근 인증/명시 삭제·2탭 로그아웃·cookie 계정 전환의 삭제 금지·8언어 화면/장애·SW/API 비캐시·IndexedDB 인증값 비저장을 실행했다. Chrome viewport emulation이며 실제 휴대폰 시험은 아니다.
- 같은 CI의 [실DB16](https://github.com/crystal23733/search-mine/actions/runs/37779475034/job/113319048497)과 원본 coverage 재계산: auth domain369/371=99.46%, whole server2750/3057=89.96%. [core](https://github.com/crystal23733/search-mine/actions/runs/37779475034/job/113319047974)2314/2366=97.80%. 실행 진입점 main/migrate도 서버 분모에 포함한다.
- Web V8 lines1444/1661=86.93%, branches977/1244=78.53%. main 포함이며 branch80% 달성을 주장하지 않는다. UTF-8 복구 source22c4c67의 초기 JS+최대 선택 locale gzip46429bytes, SW6942bytes, precache41개/1320190bytes. 초기 JS 예산과 전체 precache 용량은 구분한다.

pm-ai-shipping code-review2.1.0의 정확성/보안, scope#46·baseline develop de55e16019549ef0b523f2854b6ac4ad522b066a로 순차 검토했다. UI→AuthPort→현재 bootstrap/세대→strict HTTP→실제 권리 API→공개 DTO/화면, raw input→Rust canonical nickname, 계정 변경→확인/내보내기/대기 보존 경계를 양쪽에서 읽었다. 서버 권위와 stale cookie/다른 account를 다르게 만드는 시험, observer/역순 응답/삭제 뒤 bootstrap 교체와2탭 전환을 강제 실행했다.

지원되는 발견을 수정했다. 닉네임 요청이 서버 PATCH /me와 다른 경로였고 계약 Red 후 수정했다. 오래된 nickname 완료가 observer의 새 generation 뒤 적용되는 Red, 삭제 후 bootstrap 교체/나중 갱신에서 새 계정에 완료 안내가 붙는 Red는 소비 시점 generation 검사와 notice 수명으로 수정했다. malformed200 응답이 권위를 유지하는 Red, 초과 본문 전체 소비 Red는 unavailable/스트림 취소로 수정했다. browser redirect 예외의 안내 누락 Red도 수정했다.

최초 [실제 browser Red](https://github.com/crystal23733/search-mine/actions/runs/37777025689/job/113310357009)는 재인증 뒤 삭제 확인에서 Close focus가 inactive임을 확인했다. safe focus 수정 후 최신57 Green이다. 초기 전체 API금지 검사는 승인된 bootstrap과 충돌하여 first-party bootstrap만 허용하고 나머지 API/provider/광고 금지를 유지했다. 형식 실패/재시도/미실행을 성공으로 계산하지 않는다.

자기 서명 시험 cert는 기본 SW script fetch를 막았다. 동일 loopback probe에서 제한 없는 신뢰 설정 없이 readyfalse/SSLerror를 확인한 뒤, 공개 시험 cert SPKI만 신뢰하면 readytrue임을 확인했다. 해당 Chromium flag는 auth 시험 project에만 둔다. production main/config에 시험 proof/clock/키 선택 경로가 없다.

## 화면과 남은 범위

최신 CI의 로그인·닉네임·설정 PC1440/mobile390 이미지를 직접 읽어 reference 카드/큰 버튼/초대 없는 계정 화면·공통 grid/tokens·focus·overflow를 확인했다. 제공자 로고를 임의로 만들지 않고 텍스트 버튼을 사용한다. 공식 branding/실계정 최소 scope/키는 #42 공개 출시 검수다.

- [모바일 로그인](screenshots/46-login-390.png), [닉네임](screenshots/46-onboarding-390.png), [설정](screenshots/46-settings-390.png)
- [PC 로그인](screenshots/46-login-1440.png), [닉네임](screenshots/46-onboarding-1440.png), [설정](screenshots/46-settings-1440.png)

실제 제공자 proof/TLS/console·알림 audience·계정 철회는 #42다. 공식 제출/순위는 #19, 친구 초대 code의 전체 OAuth 왕복 보존은 #17의 서버 거래 확장이다. 이번 local onboarding URL 보존 시험은 OAuth 왕복의 보존 증거가 아니다. 정책/CMP/운영자 정보는 #21, SEO/noindex는 #23, 실기기/미니 PC 부하·사람 이해도는 #26/#27이다. 법적 출시 준수나 실제 외부 계정 검수 완료를 주장하지 않는다.
