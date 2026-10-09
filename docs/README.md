# 문서 안내

1. [제품 비전](product/01-vision.md)과 [PRD](product/16-PRD.md)로 범위·성공 기준을 읽는다.
2. [설계 리뷰·요구 추적표](design/00-review-checklist.md)에서 승인할 제안을 확인한다.
3. [시스템 구조](design/01-architecture.md), [규칙](design/04-game-rules-spec.md), [간파 알고리즘](design/05-algorithms.md), [프로토콜](design/06-protocol.md)을 검토한다.
4. [테스트 시나리오](product/19-test-scenarios.md)와 [백로그](workflow/backlog.md)에서 구현·검증 단위를 확인한다.
5. [Git workflow](workflow/git-workflow.md)와 [스킬 적용 내역](workflow/skill-usage.md)을 따른다.

## 문서 묶음

- `planning/`: 제공된 브레인스토밍·핸드오프 원문, 내용 그대로 보존.
- `product/01~21`: 비전·경쟁·세그먼트·여정·가치·경제·리스크·요구·테스트·지표·로드맵.
- `design/01~18`: 구조·배포·도메인·규칙·알고리즘·프로토콜·DB·client·UX·언어·광고·보안·성능·테스트·repo·운영·OAuth/개인정보·제공된 디자인 기준.
- `adr/0001~0031`: 공유 코어·DB·홈서버·Preact·Pixi·권위·간파 증명·workflow·최소 OAuth·공개 후보 전략·규칙 snapshot·공개 관측 봇·public WASM·Atomic shell/locale·로컬 연습/데일리·공개 cache·인증/계정 권리·온라인 actor/큐/보드/봇/매치 조립·세션/최초 연결·인증 로비 HTTP·OAuth 초대 복귀의 결정.
- `workflow/`: develop·이슈/마일스톤·승인 상태·작업 백로그·스킬 출처.

[#16 권위 actor·WS·원자 결과 검증](verification/16-authoritative-match-actor.md)과 [ADR0023](adr/0023-authoritative-match-actor.md)은 인증 철회/직렬 입력/공개 DTO/결과 transaction의 실행 근거와 후속 매칭·클라이언트·운영의 경계를 기록한다.

[ADR0019](adr/0019-auth-foundation-and-delivery.md)와 [#43 인증 기반 검증](verification/43-auth-foundation.md)은 최소 저장·원자 거래·세션·실DB/coverage 근거를 기록한다. #43→44→45→46 순차 구현, #42 실제 계정 확인의 별도 출시 조건을 따른다.

[ADR0020](adr/0020-oauth-providers-and-http.md)과 [#44 제공자/HTTP 검증](verification/44-oauth-providers-http.md), [운영 secret 설정](workflow/oauth-configuration.md)은 최소 권한·JWKS·브라우저/세션 경계와 실제 계정 검수 제한을 기록한다.

현재 M0/M1과 #4~14 구현은 develop에 병합됐고 [#14 오프라인/대기 검증](verification/14-public-offline-cache.md)을 마쳤다. 실제 사람의 E1/E2 관찰은 #27/M2 수동 게이트로 유지한다. [#10 Atomic shell](verification/10-atomic-shell.md)도 확인한다. 승인/실행 결과/가정을 구분한다. 참조 기술 문서는 각 실제 구현/공개 시 다시 확인한다.

[#13 UTC 데일리·개인 미검증 기록·공유 검증](verification/13-utc-daily-records-share.md)과 [ADR0017](adr/0017-deterministic-solo-daily-and-local-records.md)을 추가했다. 공개 캐시/안전 업데이트/제출 대기는 [#14 검증](verification/14-public-offline-cache.md)과 [ADR0018](adr/0018-public-offline-cache-and-safe-update.md)을 확인한다. 공식 기록/순위는 #19다.

[ADR0021](adr/0021-account-rights-and-apple-revocation.md)과 [#45 계정 권리·Apple 철회 검증](verification/45-account-rights-apple-revocation.md)은 명시적 연결·재인증·최소 내보내기·원자 삭제·서명 알림·일일 확인과 동시성 수정 근거를 기록한다. 실제 키/계정은 #42, 화면은 #46에서 별도 검증한다.

[ADR0022](adr/0022-web-oauth-account-ui.md)와 [#46 계정 화면·실제 HTTPS 검증](verification/46-oauth-account-ui.md)은 메모리 인증 권위·계정 전환·최소 공개 DTO·확인/재인증·8언어 화면과 실DB browser 근거를 기록한다. 실제 제공자 검수는 #42, 전체 초대 왕복은 #17, 공식 제출은 #19다.

[ADR0029](adr/0029-initial-match-connection-lifecycle.md)와 [#62 최초 연결 검증](verification/62-initial-match-connection-lifecycle.md)은 로비→actor 초기 권한 전달·Rust 시작 기한 취소·실제 WS와 남은 공개 HTTP/초대/화면 출구를 기록한다.

[ADR0030](adr/0030-authenticated-lobby-http-runtime.md)와 [#64 인증 로비 HTTP 검증](verification/64-authenticated-lobby-http-runtime.md)은 닫힌 Rust→TS intent·모든 호출의 Origin/CSRF·bounded read/열거 rate·실제 실행 조립과 남은 초대/화면 출구를 기록한다.

[ADR0031](adr/0031-oauth-invitation-return.md)과 [#66 OAuth 초대 검증](verification/66-oauth-invitation-return.md)은 서버5분 거래의 canonical code/locale·일회 consume·성공/취소/실패 복귀와 nickname/tutorial·저장소 거부를 기록한다.
