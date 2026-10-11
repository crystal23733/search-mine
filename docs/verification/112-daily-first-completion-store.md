# #112 · 데일리 게시와 첫 검증 완료 저장

M3 / 상위19 · FR09/10 · NFR01/02/05/07/08 · TS17/18/20/21/31/35/36.
선행109/PR111 developcbd3541 병합·109 CLOSED를 확인했다. [ADR0052](../adr/0052-daily-first-completion-store.md)를 제품 코드보다 먼저 작성했다.

검증 계획은 actual native canonical artifact/공백·순서·다른 payload, 실제PG immutable publication/accepted snapshot 차이·동시 publisher, 첫 완료·같은 attempt retry·변경 Conflict·Practice/다른 계정UUID, 실제 session/삭제·rollback/SQL lock, 정확한7일/90일/DST·보존 anchor/forward migration이다. 기존 도메인95/서버80·전체 회귀·문서/Mermaid·코드리뷰·최신head CI를 실행한다.

아직 제품 코드·Red/Green·실DB/coverage·PR/병합은 미완료다. HTTP/timing/순위/게임기록 export/웹·운영 성능/외부키/물리삭제는 이 foundation의 완료로 계산하지 않는다. review/2026-10-11.md는 ignored 상태다. 실제 실행 결과를 아래에 이어 기록한다.


## 실제 TDD·저장 코드와 DB 회귀

canonical evidence Red는 실제 decode/replay 뒤 getter skeleton의 빈 JSON/digest를 비교해 Exit101이었다(.tmp/daily112-payload-red.log). 실제 typed replay의 canonical JSON·SHA256을 sealed evidence에 묶고 semantic ReplayOutcome을 분리하여17개 daily 검사 PASS2.28초를 확인했다. JSON 공백/키 순서는 같은 제출이며 완료 통계가 같아도 final_time_ms가 다른 payload는 다른 digest다. 원문은 Debug에 포함하지 않는다.

PG publication skeleton Red는 실제 migration 뒤 Unavailable로 Exit101이었다(.tmp/daily112-pg-red.log). 첫 Green 시도는 SQLx0.9의 SqlSafeStr 컴파일 제한에 실패했다(.tmp/daily112-publish-green.log는 수정 후 결과). SQLx QueryBuilder에 static 조각과 bind만 사용하여1개 실제 게시 검사 PASS0.59초를 확인했다. 값 문자열을 SQL로 조합하거나 AssertSqlSafe로 우회하지 않았다.

최초 완료/동일 재전송 skeleton은 실제 DB에서 Unavailable로 Exit101·1개 실패0.64초였고(.tmp/daily112-submit-red.log), account/session의 기존 transaction authorize를 crate 내부로 공유하고 immutable definitions/first completion/own port를 구현해2개 PASS0.86초였다. auth helper의 SQL/권한 정책은 바꾸지 않았다. payload는 worker가 실제 재생한 sealed artifact에서만 얻는다.

추가 검사 초안은 async closure 수명 컴파일 오류 Exit101이었다(.tmp/daily112-pg-behavior.log). 참조 pool과 Copy 시각/계정의 소유권을 명확히 해7개 PASS1.44초를 확인했다. 이후 게시 시각보다 이른 완료 행을 정상 receipt로 읽는 실제 결함을 주입 DB 행으로 재현했다(.tmp/daily112-before-publication-red.log,0PASS1FAIL0.62초). publication 시각/검증 만료와 완료 anchor를 함께 검증하도록 고쳐8개 PASS1.58초였다(.tmp/daily112-window-green.log). 이 수정은 corruption fail-closed 계약을 강화하며 정상 writer 통계/게임 규칙을 바꾸지 않는다.

최종14개 새 actualPG 검사6.46초 PASS(.tmp/daily112-pg-lifetime.log): accepted snapshot과 다른 proposal/동시 publishers, accepted artifact만 저장, canonical retry·통계 동일 다른 입력 Conflict·다른 날짜 Conflict·Practice·다른 계정 같은 attempt, 동시 first/retry, exact7일/90일, nickname/실제 session·logout·FK삭제/late retry, 실제 SQL lock·caller cancellation·pool acquire deadline·injected INSERT failure rollback, adapter 재생성과 응답 유실 재전송, 대기 중 clock/만료 재검증, concurrent 철회/삭제, forward migration의 auth+online journal 보존, America/New_York DST를 건너는 elapsed seconds, immutable anchor/통계·raw NULL만 허용·복구 거부, 크기/type/digest/정수 초 CHECK code23514, 손상 snapshot/ID 읽기 거절을 확인했다. SQL lock 관측은 pg_blocking_pids로 실제 대기한 뒤 release/revoke/delete/clock 변경을 수행하며 단순 sleep을 성공 근거로 쓰지 않는다.

전체 actualPG17.4 회귀75개10.61초/ignored0 PASS(.tmp/daily112-full-pg.log), migration 명령 Exit0(.tmp/daily112-migrate.log), all-targets Clippy -D warnings Exit0(.tmp/daily112-clippy.log), daily 도메인/검증기18개2.31초 PASS(.tmp/daily112-domain-green.log). docs162/product21/design18/trace36 및 Mermaid34/실패0·git diff --check PASS다. Daily domain gate는 model.rs+records.rs 두 파일을 요구하도록 확장했다. 새 per-issue coverage와 전체 scripts/check.ps1·최신CI·PR/병합은 아직 미완료다.

제품 파일: daily/model.rs·replay.rs는 입력과 실제 완료 evidence의 결합, records.rs는 비밀 session과 receipt/저장 port·도메인 검증, postgres.rs는 게시 권위/기존 auth 잠금/최초 완료와 본인 조회·2초 전체 기한, migration은 unique/FK·불변 trigger/보존 anchor다. 실제 PG 행동은 tests/postgres/daily.rs와 daily_storage.rs, 도메인 음성 경계는 tests/unit/daily_records.rs에서 검사한다. 의존성/lockfile·게임 규칙·프로토콜/웹·main 활성화는 변경하지 않았다.

pm-ai-shipping:code-review correctness/changes, baseline develop cbd3541: proposal→committed snapshot→실제 verifier→sealed payload→account/session lock→unique first row→retry/own read→erase cascade의 계약을 함께 검토했다. 요청 hash와 accepted hash를 강제로 다르게 했고 두 publishers/first submit/retry·철회/삭제를 겹쳐 검사했다. 틀린 게시 이전 receipt는 실제 Red→Green으로 해결했다. write/권한은 같은 transaction·account→session lock 순서이고 삭제 후 artifact는 계정을 복원하지 않는다. retry는 동일 digest/통계·원래 anchor를 확인하며 다른 attempt의 Practice는 새 payload row를 쌓지 않는다. DB 취소/SQL 오류는 success로 바꾸지 않으며 응답 유실은 같은 attempt로 찾는다. 현재 범위에서 근거 있는 추가 미해결 correctness 결함은 확인하지 못했다. 종합 보안/성능 감사나 모든 버그 부재를 주장하지 않는다.

raw replay의7일 물리 삭제·backup/definition FK 수명 관리는25의 출시 조건이다. 논리 own은90일 경계에서 숨기지만 이 단계에서 물리 삭제를 수행했다고 주장하지 않는다. main/HTTP는 새 수집 port를 연결하지 않으며 다음 필수 game export/HTTP proof·시간신뢰/순위·웹 이전에 공식 수집을 켜지 않는다. 상위19/미해결83/외부42·26·27은 OPEN이다. review/2026-10-11.md는 ignored로 유지한다.


## 새 빌드 실제 커버리지와 누락 방지

전용 새 CARGO_TARGET_DIR=.tmp/daily112-coverage-clean에서 실제PG 포함 coverage를 실행해 Exit0을 확인했다(.tmp/daily112-coverage.log/json은 .tmp/daily112-server-coverage.json). Daily model+records104/104=100%, 전체 서버7313/7836=93.33%, Auth99.48/Match99.31/Lobby98.37/Result100%로95/80 gate를 통과했다. 기존 entry main/migrate와 새 PG adapter를 전체 서버 분모에 포함하며 exclusions를 추가하지 않았다. 같은 보고서에서 records.rs만 제거한 음성 보고서는 gate가 missing domain으로 Exit1을 반환했다(.tmp/daily112-missing-domain.log). 누락된 새 도메인 파일을 기존 model 수치로 대체하지 못한다.

추가 문서 리뷰에서 예전 conceptual DAILY_ATTEMPT/partial unique ERD가 ADR0052의 완료 한 행 저장과 맞지 않아 설계07의 DAILY/DAILY_COMPLETION 필드·실제 테이블 mapping·unique/삭제/보존을 현재 계약으로 갱신했다. 공개 seed를 중복 보관하거나 아직 없는 verified_time/순위 구현을 표현하지 않는다. 전체 scripts/check.ps1·최신 CI·PR/병합은 아직 미완료다.


## 최종 로컬 전체 검사

scripts/check.ps1 실제 프로세스 Exit0(null ExitCode 거부) 확인: fmt/Clippy all-targets·335Rust·WASM 컴파일/생성 types·fixture·token freshness·format/lint/typecheck·211unit37파일/web line88.25%·production build·96browser3.3m/재시도0·docs162/Mermaid34실패0 PASS다(.tmp/daily112-full-check.stdout.log/stderr.log). fresh coverage의 실제PG75개10.75초/ignored0도 확인했다. code와 test는 검사 중 수정하지 않았다.

최종 diff를 baseline developcbd3541과 다시 검토했다. canonical artifact·PG first completion/권한·불변 migration/도메인 gate와 행동 검사, 설계/추적 및109 실제 종료 기록만 포함한다. main/HTTP/웹·규칙·의존성/lockfile·보존 원문·design_layout은 변경하지 않았다. review/.tmp는 ignored이며 커밋하지 않는다. PR 제출/최신 head의 필수 CI·squash merge/112 CLOSED는 아직 미완료다. 상위19는 OPEN이다.
