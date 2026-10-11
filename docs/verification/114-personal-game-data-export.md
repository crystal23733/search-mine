# #114 · 본인 게임 기록 내보내기

M3 / 상위 #19 · FR01/09/10/16 · NFR01/02/05/07 · TS17/18/20/29/31/34/35/36.
선행 #112/PR113은 develop `311ab660c861f31e36f11e8422e795020a3287d8` 병합과 CLOSED를 확인했다. 제품 코드 전에 [ADR0053](../adr/0053-personal-game-data-export.md), 개인정보·프로토콜·DB·테스트 설계와 추적표를 작성했고 제공된 설정 디자인13을 확인했다.

## 변경 결과

기존 설정의 명시적 내보내기는 최소 계정 정보와 본인 온라인·데일리 완료 기록, 살아 있는 본인 daily replay를 버전1 `PersonalDataExport` 파일로 모은다. 닉네임 없는 계정도 사용할 수 있다. 추가 OAuth 권한·비밀번호·profile 필드를 수집하지 않는다. 무계정 연습과 대기 기록은 자동 귀속하지 않는다.

- `crates/server/src/privacy/`: domain/reader port, 실제PG adapter, 전용 authority/rate와 동시2 worker, 고정 POST `/api/v1/me/games/export`·`/api/v1/me/games/replay`.
- `crates/protocol/src/privacy.rs` → 생성 TS: 닫힌 요청·본인 projection·파일 DTO. session hash/subject/digest/credential과 숨은 online seed·board·상대 통계는 없다. daily metadata의 공개 seed는 기존 공개 계약이다.
- `apps/web/src/services/auth/game-export.ts`·`session.ts`·`browser.ts`·`AccountSettings.tsx`: 같은 account/session proof로 계정→페이지→raw를 모으고 마지막 bootstrap/generation 확인 직후 기본 동기 download sink를 실행한다.
- 기존 result projection, accepted daily publication/receipt decoder, account→session transaction authorize와 bounded read rate 알고리즘을 공유한다. counters/authority/worker는 gameplay 소유권과 분리하며 main과 HTTPS fixture의 invalidation에 연결했다.

페이지는 UUID ASC 50개+1개로 다음 여부를 결정한다. strict body 4KiB·페이지64KiB·현재 세션과 최근300초 인증·전체 요청2초를 적용한다. daily raw는7일, record는90일이며 online은 기존 admitted/legacy anchor의90일을 따른다. 만료 raw는 물리적으로 남아 있어도 SQL CASE에서 본문을 읽지 않는다. CPU worker는 caller 취소/timeout 뒤에도 실제 종료까지 permit을 소유한다. response는 no-store/no-referrer/nosniff다.

브라우저는2048페이지/32MiB 안에서 수집하고 요청 사이100ms를 둔다. 오류·계정 전환·늦은 응답·취소를 완성 파일이나 정상 null로 바꾸지 않는다. raw의 정상 만료/삭제만 null이다. 각 페이지는 실제 읽기 transaction이며 여러 HTTP 요청에 하나의 DB snapshot을 보장하지 않는다.

## 실제 검증 · 2026-10-11

| 검사 | 결과 |
|---|---|
| `scripts/check.ps1` | 실제 종료 코드0. fmt/Clippy all-targets -D warnings·WASM·타입/fixture/token 최신성·format/lint/typecheck·unit/build/E2E·문서/Mermaid 통과 |
| workspace Rust | 347개 통과. 이 실행의 DB83개 ignored는 별도 actualPG 실행으로 검증 |
| 실제 PostgreSQL17.4 전체 회귀 | 83개 통과, 11.66초, ignored0 |
| privacy actualPG | 8개 통과, 4.63초 |
| privacy model/worker | 6개 통과, 2.01초 |
| privacy HTTP | 6개 통과, 4.05초 |
| 웹 전체 unit | 38파일·220개 통과, 줄 커버리지88.83% |
| 실제 브라우저 | 98개 통과, 3.5분, 재시도0 |
| 새 target 서버 coverage | actualPG83개 통과, 12.39초. Privacy160/160=100%, 전체7869/8433=93.31% |
| 기존 domain gate | Auth99.48%·Match99.31%·Lobby98.37%·Result100%·Daily100%, 모두95% 이상 |
| coverage 누락 검사 | Privacy domain을 제거한 보고서가 Exit1로 거절됨 |
| 문서/Mermaid | Markdown164·제품21·설계18·시나리오 추적36 / Mermaid34개, 실패0 |

실제PG는51개 online/daily keyset, 타인·cursor·세션revision·닉네임 없음·exact300/7일/90일, 물리 raw 잔존 필터, digest 손상/NULL, 계정 삭제 cascade를 검사했다. `pg_blocking_pids`로 잠금 대기를 확인한 뒤 clock 변경·삭제·철회, pool/SQL 실패·timeout·취소를 검사했다. 다른 timezone의 같은 결과도 확인했다.

HTTP는 입력 allowlist/UUID/version/body/Origin/CSRF/query, 요청 capacity·20/s rate·2초 기한, 전후 실제 session port proof·invalidation barrier와 잘못된 port 응답을 검사했다. worker 테스트는 둘을 실제로 막아 caller 취소와 timeout 뒤 세 번째 CPU 작업이 거절되는 것을 확인했다.

브라우저는 production PG/router를 사용한다. 새 E2E는 실제 대전을 완료한 본인의 통계만 PC1440px/mobile390px에서 각8언어로 다운로드하고 파일의 공개 필드와 비밀 제외를 검사한다. 외부 OAuth proof는 시험 port로 대체한다. 기존 제공자 연결/해제·계정 전환·offline·WS·OS restart 회귀도 통과했다. 실기기 측정과 실제 제공자 검증을 대신하지 않는다.

## Red → Green과 코드 리뷰

최초 PG 미구현은0/1 실패0.44초, 미연결 HTTP는404 대503으로0/1 실패, 기존 auth-only 브라우저 export는 games/replays/v 부재로0/1 실패했다. 각각 구현 뒤 통과했다. 좁은 web 검사는 전체 coverage와 분리하고 전체220개 실행의88.83%를 보고했다.

correctness/changes 범위를 기준 develop311ab66과 비교해 요청 proof→실제PG/accepted receipt→CPU→post-proof/barrier→browser generation/sink를 검토했다. 실제 finding은 reader 완료18002 뒤 최종 proof18001을 시작18000과만 비교해200을 반환한 것이다. `.tmp/privacy114-clock-review-red.log`에서0/1 실패를 확인했고 read_at까지 역행을 검사한 뒤 HTTP6개가 통과했다. 정상 대용량 페이지 순회의 자체 rate 문제는 abort 가능한100ms pacing으로, inherited UI의 account ID만 보는 sink 간격은 `exportTo`의 현재 completion 확인과 동기 sink 호출로 보완했다.

중간 실패를 숨기지 않는다. 최초 PG 테스트 입력은30일 session anchor 제약과 통계23514에 막혔고, 제약을 유지한 채 가능한 입력으로 수정했다. worker test의 동기 채널 대기는 async 채널로 바꿨다. 첫 전체 검사219pass/1fail은 예전 AuthExport 기대값이었고 새 파일 계약으로 갱신한 화면11개와 전체220개가 통과했다.

첫 전체PG 회귀는82pass/1fail(main admission4초 timeout), 단독 진단은1pass였다. 해당 실행의 원인은 상태 로그가 없어 확정하지 않는다. 별도로 기존10ms HTTP poll이20/s 계약을 넘길 수 있는 문제를 확인해100ms로 바꾸고 HTTP 오류를 즉시 드러냈다.4초 예산·재시도0은 유지했고 수정 후 전체83개와 fresh coverage83개가 통과했다. 첫 실패 로그는 로컬에 보존했다. #83의 별도 두 탭 SW 문제를 해결했다고 계산하지 않는다.

## 범위와 남은 절차

로컬 구현·리뷰·관련 검사와 full check는 완료했다. 원격 PR/최신 CI·exact-head squash merge·#114 CLOSED 확인은 다음 절차다. 일별 실제 리뷰/로그는 Git에서 제외한 `review/2026-10-11.md`에 기록한다.

공식 데일리 HTTP 수집/time ticket·heartbeat·두 순위·명시 제출 UI는 다음 #19 하위다. 이 작업은 verified_time이나 인간 풀이 증명을 만들지 않는다. 물리 cleanup/backup #25, 실제 OAuth #42, 하드웨어 #26, 사람 관찰 #27, 미해결 #83과 운영자 정책 입력은 별도다.
