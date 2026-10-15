# #140 원시 이벤트 30일 물리 보존

[이슈140](https://github.com/crystal23733/search-mine/issues/140) · [ADR0066](../adr/0066-bounded-raw-event-retention.md) · 상위20/M3.

선행138 CLOSED·PR139/develop3d53a20 병합, 검증 source와 merge tree 동일·clean/0ahead/0behind를 확인하고 feature/140-raw-event-retention으로 분기했다. 제품 코드 전에 occurred_at 원래30일·500개/transaction·2초·SKIP LOCKED·별도 maintenance/권한을 정의했다.

| 경계 | 관측할 결과 |
|---|---|
| 원래 시각→삭제 | 정확30일 전후, received_at/재시도로 기간 연장 없음 |
| batch/동시성→DB | 500+1·concurrent worker·잠긴 row skip/backlog, 각transaction2초 |
| failure/Clock→원자성 | commit 전 SQL 오류·역행·timeout/cancel은 rollback, COMMIT 응답 불명은 unavailable/다음 주기 확인 |
| 삭제→본인 권리 | 논리 만료 즉시·물리 삭제 구분, 원본 게임 유지·raw 재생성0 |
| port→main | 즉시/60초·missed tick skip·cycle8batch/2초·진전0 중단·게임요청과 별도 |
| 제한 DML→권한 | 필요한 row lock UPDATE(id)만·불변 trigger/다른UPDATE·DDL차단 |

익명 calendar13개월·first-normal/NSM·D7·운영 조회는 다음 필수20이다. 현재 없는 집계를 정리했다고 보고하지 않는다.

## 실제 실행과 검토

[PG 보존 시험](../../crates/server/tests/postgres/event_retention.rs)의 expired_raw가 실제 committed source/event를 만든 뒤 stub ErrUnavailable(expected deleted1)로 Red0pass/1fail/0.53초였다. 최소 구현 뒤 같은 시험1pass/0.50초. 이후 정확30일 전후·received 비연장·500+1/미래row 보존·동시 worker·잠긴row skip·SQL 두번째 delete 실패/Clock 역행 rollback·pool/SQL deadline/취소·timeout 설정 비누출·actual main startup과 본인 export를 확인했다. 초기6개는4pass/2fail이었으며 expired session이 startup authcleanup으로 삭제된 준비와 timeout 후 이전 backend PID를 재사용한 준비를 바로잡아6pass/3.54초였다. 다중 Update patch 구문 실패는 편집 없이 재적용했고 행동 Red로 계산하지 않았다.

실제 제한 DML 역할1pass/0.72초: SELECT/INSERT/DELETE만으로 저장되지만 FOR UPDATE는42501, UPDATE(id)만 추가하면 cleanup 성공·다른 column UPDATE42501·ID 변경은 불변 trigger23514, 삭제 뒤 원본 retry는Duplicate/raw0이었다. [maintenance port](../../crates/server/tests/unit/event_maintenance.rs)4pass는8batch·공유2초·부분실패/잘못된count·진전0·즉시/60초·missed tick skip·실패 뒤 다음 주기·작업 겹침0을 확인한다.

첫 전체PG124pass/1fail/20.44초는 기존 [141](https://github.com/crystal23733/search-mine/issues/141)의 실제main cache ready 뒤4초 preparing 지연이었다. 별도 fresh coverage는PG125/20.28초·서버9896/10528=94.00%·Event99.70%·Privacy100%·domain95/전체80을 통과했다. prepare/register started/ready의 안정 진단을 추가했으며 성공 반복으로 원인 해결을 주장하지 않는다. 추가 계정삭제 cascade 경합1pass/0.50초는 만료된 다른 계정만 삭제하고 잠긴 본인 row를 건너뛴 뒤 계정삭제 commit/재전송 비복원을 확인했다. 최종 동일 소스 전체PG126pass/20.13초였다. 최신 진단/추가시험의 fresh coverage 수치는 별도 갱신한다.

로컬 scripts/check.ps1 exit0: fmt/clippy·Rust420pass(당시125ignored DB는 별도 실행)·WASM/types/fixtures/tokens/format/lint/typecheck/build, 웹44files/267pass·줄90.91%, 실제Chromium113pass/5.8분·재시도0, docs190/제품21/설계18/시나리오36·Mermaid34/failure0. 이후 추가된 계정삭제 경합도 별도 PG 전체126에서 확인했다.

정확성 changes 리뷰 기준3d53a20: 원래 occurred_at 경계→SQL 대상,500개·SKIP LOCKED→동시/삭제 cascade, client/DB 남은 기한→rollback/불명 commit, 마지막 IO 뒤 Clock 재검사→commit, 최소 UPDATE(id)→불변/권한, cycle8/2초→main 별도 작업과 로그·권리/재전송을 대조했다. 이 범위에 추가로 뒷받침된 제품 결함은 없었다. 미확정 준비141·운영 부하26/백업25·실OAuth42/사람27·offline83을 이 출구로 완료했다고 주장하지 않는다. 원격 필수6checks/정확head 병합/CLOSED는 PR과 일별 로컬 리뷰에 실제 결과로 기록한다.

최종 동일 소스 fresh coverage는441pass(실제PG126/20.04초), Event335/336=99.70%·Privacy182/182=100%·서버9904/10536 lines = 94.00% (required 80%)이며 모든 domain95/전체80 게이트를 통과했다. 기록 위치는 로컬 events140-final-20261016014955이고 이전125개 보고서와 구분한다.
