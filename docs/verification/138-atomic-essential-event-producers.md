# #138 서버 결과와 필수 이벤트 원자적 수집

[이슈138](https://github.com/crystal23733/search-mine/issues/138) · [ADR0065](../adr/0065-atomic-essential-event-producers.md) · 상위20/M3.

선행136 CLOSED·PR137/develop3a1e0be와 clean 최신 develop을 확인하고 feature/138-atomic-essential-event-producers로 분기했다. 제품 코드 전에 같은 source transaction·UUID순 account 잠금·재전송 비복원·unknown 복구 의미를 정의했다.

| 경계 | 관측할 결과 |
|---|---|
| direct/runtime/journal→event | 실제 source와 살아 있는 인간 각각1건이 함께 commit, bot0·민감값0 |
| 저장 실패→rollback | event INSERT 거절 시 결과/완료 INSERT와 active 삭제 모두 rollback |
| retry/Practice→identity | 최초 ID/received_at 유지, raw 삭제/만료 뒤 재생성0 |
| 잠금/삭제→동시성 | 계정 UUID순·recovery batch 전체순서, account 삭제 비복원 |
| 실제 OS restart→unknown | 미확정 server_failure는 자동 수집하되 정상완료/NSM 제외 |
| 실제 HTTPS→본인 다운로드 | 수동 capture route 없이 PC/mobile8언어 own online/event·secret 제외 |

## 실제 실행과 검토

[실제 PG producer](../../crates/server/tests/postgres/event_producers.rs)의 direct 결과 ACK는 성공했지만 자동 event0건(expected2)인 assertion Red0pass/1fail/0.51초를 실행했다. 구현 뒤 같은 테스트1pass/0.48초, 확장8개는4.92초에 통과했다. 두번째 event 실패 후 결과/참여자/첫event0건, 반대좌석·중복원본 동시저장, 원래30일/삭제 비복원, journal finish/startup 실패 공동 rollback·최초 accepted ID, 원래90일 만료, daily first/plain/sealed/retry/Practice·완료와 replay rollback을 확인했다. sealed helper 인자 순서 컴파일 오류는 수정했으며 행동 Red로 계산하지 않았다.

[실제 main OS kill/restart](../../crates/server/tests/postgres/admission_restart.rs)는 unknown server_failure/human fact와 원래 recorded_at/received_at·두번째 재시작 accepted ID를 확인한다. [journal](../../crates/server/tests/postgres/journal.rs)은 삭제된 피어 뒤 남은 인간의 timeout/human과 같은 transaction 시각을 확인한다. 제한 DML 역할은 product_events SELECT/INSERT/DELETE만으로 자동 결과+event 저장/계정삭제 cascade가 가능하고 event UPDATE/DDL/migration 권한이 필요 없음을 검증한다.

로컬 격리 PostgreSQL17.4 전체119pass/20.51초, 최신 진단 추가 뒤119pass/18.45초. 별도 fresh llvm-cov430pass(실DB119/21.64초), Event335/336=99.70%·Privacy182/182=100%·Daily877/893=98.21%·서버9793/10416=94.02%이며 나머지 domain95/전체80 게이트도 통과했다.

처음 PG117pass/2fail/19.60초에는 제한 DML fixture의 새 테이블 권한 누락과 main pair준비4초 timeout이 있었다. 최소 권한을 추가했다. main 단독1pass/3.06초·이후 전체119pass였으나 다른 전체에서도 pair준비 실패가 반복됐다(118pass/1fail/18.53초). 마지막 공개 상태/명시 failed 진단을 추가하고 시간 상한은 유지했다. 최신 전체119pass이며 준비 실패 원인을 확정하지 않는다. 최초 브라우저 전체86pass/1fail/26미실행·3.4분은 새 daily 기대값을 name으로 적은 오기였다. Rust DTO의 type으로 수정했다. 수동 capture 없는 online export PC/mobile은 각각6.9/6.6초에 통과했다. 최종 전체/원격 결과는 완료 뒤 추가한다.

정확성 changes 리뷰(기준 develop3a1e0be)는 모든 writer의 source INSERT→collector→commit, account→participant/source 잠금·recovery batch 전역순서, 실패→공동 rollback, Duplicate/Practice→기존 ID/시간·raw 비복원, unknown→정상완료 제외, source→최소 typed fact, optional 차단과 실제 자동 HTTPS 관측을 대조했다. 이 범위에서 추가로 뒷받침된 제품 결함은 없었다. main 준비 실패는 미확정으로 구분한다.

물리 retention·anonymous 집계·first-normal/D7·운영 조회는 후속20이다. 원격CI/정확head 병합 상태는 해당 PR/이슈와 로컬 일별 리뷰에 실제 결과로 기록한다. 실OAuth42·운영25/26·사람27·미확정83과 실제 성능 목표를 이 producer로 완료했다고 주장하지 않는다.

최종 scripts/check.ps1 exit0: fmt/clippy·Rust416pass(119ignored DB는 별도 실제 실행)·WASM/types/fixtures/tokens/format/lint/typecheck/build, 웹44files/267pass·줄90.91%, 실제Chromium113pass/5.4m·재시도0, 문서188/제품21/설계18/시나리오36·Mermaid34/failure0·diff --check 통과. PC1440/mobile390 각각8언어의 자동 online event와 plain/sealed daily event가 실제 명시 다운로드에 포함됐고 secret/replay 권리를 유지했다.
