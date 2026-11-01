# #174 · 인증 세션별 현재 동의 저장·철회 기반

FR01/13/16·NFR01/02/05/07/08·TS20/21/24/29/31/35/36 → [ADR0080](../adr/0080-session-consent-authority.md).

선행 #20/PR146·#156/PR157의 병합·CLOSED와 최신 develop e374cf4를 확인했다. 제품 코드보다 먼저 설계/추적표/백로그를 반영한다. 실제 Red/Green·코드 리뷰·검사/CI 결과는 실행 후 기록한다. 실제 CMP/본인 export HTTP/선택 이벤트 저장·상위21과83은 별도다.

## 행동 검증과 코드 리뷰

최초 unavailable 구현에서 도메인3개 모두 실패했고 실제 PostgreSQL 행동2개도 실패했다. 이 Red의 로그와 초기 성공 로그를 ignored `.tmp/consent174-*`에 보존했다. Green 구현 뒤 목적 분리·최초 철회·늦은 revision 충돌·증거 subject/기간·시계/정수 경계를 단위5개로 확인했다.

실제 PG18에서는 본인 read/타 세션 거부·기본 verifier 차단·동시 최초 저장의 승자1개·두 목적 동시 철회·검증 대기 중 철회/logout/계정 삭제/역행/증거·세션 만료·실제 account 잠금2초·로그인 세션 회전 cascade를 확인했다. verifier IO 동안 DB 잠금을 놓아 철회가 먼저 완료할 수 있다. 마지막 시각 확인까지 증거가 만료되면 INSERT도 rollback한다.

리뷰에서 preflight의 마지막 관측 시각이 후속 verifier 전 검사에 전달되지 않는 점을 보완했다. SQL 조회 뒤 시각을 확인하고, 시간열1100→1200→1199를 실제 PG 회귀로 거절한다. 검증 IO 전후와 commit 전의 session/account를 같은 잠금 순서로 다시 확인한다. CAS는 account 잠금 하에서 읽은 현재 revision과 비교한다. 임의 UUID/hash, token·TC 문자열 저장, 공개 grant API, main에 테스트 verifier를 주입하는 경로를 추가하지 않았다.

최종 로컬 PG18 일반 회귀는157개/14.33초 통과했다. 도메인5개·workspace fmt/clippy·WASM check·Rust→TS freshness·문서219/추적36·Mermaid34/실패0도 실행했다. 전체 Rust/CI의 최종 head·coverage 결과는 완료 후 추가한다. 로컬 첫 workspace 실행의 loopback bind EPERM은 샌드박스 제한이었고 실제 TCP 권한으로 재실행했다. PowerShell이 없는 로컬에서는 check.ps1 자체를 실행했다고 보고하지 않는다.

## #83 확인과 별도 한계

2026-11-01 기존 pointer CLI를 변경 없이 Chrome153.0.8010.12/revision971a7443에서 실행했다. PC/mobile control2/2는 reload, 입력군2/2는 실제 old 재시작과 원래10초 timeout이었다. 입력은 각각8회 완료했고 Warm Up 요청은 각2회였다. native 수집은 control1건이 불완전해 해당 목적 count를 null로 유지했다. 자료는 ignored `.tmp/sw-restart-hBEMkh`다. 앞선 #161의 재현 근거를 재확인한 것이며 제품 복구 완료가 아니다. 같은 revision의 공개 Chromium SkipWaiting 구현은 pending 요청이 최초1개일 때 활성화 판단을 호출한다. 같은 요청 반복이 안전한 복구라는 근거는 없으므로 제품 timeout/retry/브라우저 설정을 바꾸지 않았다. #83은 OPEN이다.
