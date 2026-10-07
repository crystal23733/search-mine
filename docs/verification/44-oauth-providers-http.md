# #44 네 OAuth 제공자와 인증 HTTP 검증

FR01/16, NFR01/02 · TS01/20/31/32/33 · M3 · 선행 #43 PR47 병합/종료 · [PR48](https://github.com/crystal23733/search-mine/pull/48). 설계는 [ADR0020](../adr/0020-oauth-providers-and-http.md), [운영 설정](../workflow/oauth-configuration.md)을 따른다. 근거 데이터는 [auth-http-summary.json](auth-http-summary.json)이다.

## 확인한 동작

Google/Kakao openid·S256, Apple scope 없는 code/query와5분 ES256 assertion, Naver code/state 서버 POST와 profile id만 매핑한다. 서명·RS256 allowlist·고정 issuer/audience와 검증 후 nonce/azp/exp/iat/nbf/at_hash를 검사한다. Google 공식 bare issuer는 canonical namespace로 digest한다. 공개 계정은 id/nullable nickname, session revision은 UUID뿐이며 subject/credential/token/hash/추가 profile은 DTO/DB/log에 넣지 않는다.

추가 공식 명세 리뷰에서 최초 callback allowlist가 Google의 정상 iss/scope 응답을400으로 거절하는 호환성 결함을 발견했다. canonical issuer+scope/authuser/prompt fixture로 실제400/303 Red를 확인한 뒤 수정했다. Google response iss 필수/canonical, 다른 provider의 존재하는 iss도 exact 고정 issuer를 검사하고 metadata는 버린다. 올바른 Google 응답은303+cookie, 틀린/누락 iss는 소비 후303 auth_failed+cookie없음을 검사한다. 이 수정 후의 실제 검증/최신 head를 별도로 확인한다.

bootstrap/start/callback/me/nickname/logout은 Secure·HttpOnly·SameSite=Lax·Path=/ cookie, no-store/no-referrer, exact Origin·5분 HMAC CSRF를 사용한다. 익명 bootstrap은 계정을 만들지 않는다. 제공자/브라우저/state/intent/time을 원자 소비한 뒤 서버에서 신원을 검증하고 세션을 회전한다. 취소/교환 실패도 소비하며 허용8locale/return enum만 이동한다. 별도 migration002로 locale를 묶고 기존 migration001은 보존했다.

JWKS5분 cache/64keys/128KiB, unknown kid의 coalesced 1회 강제 갱신과30초 제한, 일반 응답64KiB,10초 upstream deadline, redirect 금지·고정 endpoint·bounded ephemeral rate를 적용한다. 키 없는 실행은4개 비활성/null account/null csrf이고 로컬 연습/health를 제공한다. 부분/잘못된 secret 설정은 값을 공개하지 않고 거절한다.

## 실제 실행

- authorize/identity/assertion 실제 Red3개→Green, exchange Unavailable Red→Green, locale en/ko 불일치 Red→Green, bootstrap503/configNone Red→Green, stalled transport11초 outer timeout Red→10초 안정 오류 Green. compile 오류 자체를 행동 Red로 세지 않았다.
- 제품 소스4da5b8f58118be22e4f95849708e4731a87cdd94에서 `scripts/check.ps1` exit0: fmt/clippy/workspace/WASM/생성 타입·fixture/tokens, web format/lint/type/Vitest64/build, production PC/mobile/offline Chromium50(59.9s), docs96/Mermaid34. 이후 제품 코드는 바꾸지 않고 HTTP reverse callback/NFC nickname/quota·중복 cookie·크기 경계3개를 추가해 HTTP8/clippy를 재검사했다.
- [PostgreSQL18 CI](https://github.com/crystal23733/search-mine/actions/runs/37664043282/job/112938577538)에서 별도 migration CLI와 PostgreSQL5개 실제 통합이 통과했다(ignored0). HTTP+실DB locale 저장/원자 소비/같은 계정 재로그인/session UUID 변경/이전 token 철회를 포함한다. 로컬 DB5는 ignored로 남겼고 실제 실행으로 계산하지 않는다.
- 같은 CI의 llvm-cov JSON 원본을 다운로드해 재계산했다. 도메인(model/service)277/278=99.64%, 전체 서버1710/1924=88.88% lines. main/migrate를 제외하지 않았고 그 entry point coverage0도 합산했다. branch coverage 목표/성능/실계정 성공을 주장하지 않는다. 이 수치는4da5b8f 소스와 당시 HTTP5 테스트의 근거이며 추가 테스트 뒤의 새 측정값으로 표기하지 않는다.

## 코드 리뷰와 제한

pm-ai-shipping code-review2.1.0, correctness/security, 비교 develop b7d5b36 기준으로 순차 리뷰했다. browser cookie/Origin/CSRF→거래 SQL→provider signature/claims→namespace digest→session atomic rotation→cookie/DTO, secret env/file→keyring/Apple assertion, network→bounded JWKS/cache/response→whitelist까지 양쪽 경계를 읽었다. authority는 NFC nickname을 저장한 뒤 재조회한 응답, correlation은 서로 다른 locale의 거래를 역순 callback한 결과로 강제 검사했다. replay/cancel/upstream stall/SQL 회전 rollback과 closed pool은 실제 테스트 반증 근거다. 검토 범위에서 미해결의 지원되는 정확성·보안 결함을 발견하지 않았다. 전체 코드가 무결함이라는 뜻은 아니다.

실계정/등록 키/최소 console 권한·실제 제공자 TLS 교환 #42, Apple credential 저장/명시 연결/권리/알림/철회 #45,8언어 로그인/계정 화면 #46, 약관/선택 동의 운영자 법적 고지 #21은 별도다. 신원 서명 fixture는 새로 생성한 공개 테스트 키이며 실제 앱 키가 아니다. HTTP 계약 DI를 운영 로그인 우회 경로로 제공하지 않는다. 분산 공격/미니 PC 성능/운영 역프록시 로그/실기기/실제 사람 관찰은 아직 검증하지 않았다.
