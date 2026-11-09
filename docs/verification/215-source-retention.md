# #215 원본 보존 정리 검증

FR01/09/14/16 · NFR01/02/05/07/08 · TS15/16/17/18/20/21/28/29/31/35 → [ADR0099](../adr/0099-source-retention.md).

## 인수 시나리오

- 정확7일 직전 seed/replay는 보존되고 경계에 NULL이 된다. 나머지 row/digest/시각은 유지한다. 동일 제출/저장 재시도가 지운 비밀을 복원하지 않는다.
- 정확90일 직전 전적은 보존되고 경계에 source/참여/연결 raw가 삭제된다. NULL anchor fallback과 최근 저장/오래된 anchor를 함께 확인한다. 아직 참조하는 daily definition은 남기고 마지막 참조/검증 만료 후에만 정리한다.
- source cascade로 아직30일 미만 raw가 삭제될 때 같은 transaction에서 coverage를 max(기존값,occurred_at+1)로 전진시킨다. 오래된 raw만 지울 때는 coverage를 불필요하게 바꾸지 않는다. singleton 누락/trigger 오류/Clock 역행/lock timeout은 source와 coverage를 함께 rollback한다. 기존 익명 expiry·collection 시작은 유지한다.
- 각 단계100/합계500 이하·후속 batch/has_more·잠긴 source 건너뛰기·동시 capture/계정 삭제·최종 저장 경합을 실제 PG로 검사한다. transaction timeout은 pool에 누출하지 않는다. 기존 maintenance의2초/8batch·풀5+2+1을 유지한다.
- 실제 main의 journal final은 seed 정리 후 duplicate가 NULL을 유지하고90일 원본 정리 후에는 재생성하지 않는다. 만료 daily 재제출도 원본을 되살리지 않는다.
- 자동 maintenance 조립, 기존 serving grants로 실행 가능, 관련 domain95/whole80와 전체 제품 CI·문서/Mermaid·실제 코드 리뷰를 확인한다.

## 실행 기록

시나리오/ADR을 구현 전에 기록했다. 실제 PostgreSQL18에서 초기 no-op adapter의3시나리오가 모두 assertion 실패(0PASS/3FAIL)하는 Red를 확인했다. 로컬 `.tmp/retention215-red.log`에 보존했다. 구현 뒤 기본3→경합/복구9→데일리 재제출/시각/최소 권한12개 Green을 확인했다. 미래 raw 시각도 삭제·coverage를 commit하지 않는다.

- 정확7일 seed/replay NULL과 digest/receipt/원래 시각 유지, 같은 daily 제출의 replay 비복원,90일 원본·마지막 definition 삭제와 만료 재게시/재제출 거절을 실제 저장소로 확인했다.
- NULL fallback·늦은 저장/원래 anchor, online/daily 양쪽 fresh raw cascade의 같은 transaction coverage+1/GREATEST, 이미30일 만료 raw의 coverage 유지, 실제 달력13개월 expiry 유지와 metric reader Incomplete를 확인했다.
- singleton 누락·삭제 trigger 실패·Clock 역행·100ms 실제 cascade 잠금 예산에서 source/raw/coverage rollback과 timeout pool 비누출을 확인했다. FK insert transaction 동안 source가 SKIP LOCKED로 유지되고 commit 이후 새 raw를 포함해 정리되는 것도 확인했다.
- 1001개 source에서 단계100/8batch 제한과 후속 완료, journal final의 seed 비복원/정리된 final 재생성 거절, 실제 main의 자동 maintenance/closed diagnostic을 확인했다.
- NOLOGIN 최소 DML 역할로 성공하고 DDL·anchor·expiry·raw payload UPDATE가 거절됐다. 기본 RR 연결에서도 transaction-local READ COMMITTED로 실행하고 기본 설정을 누출/변경하지 않았다. 실제 grants를 넓히지 않았다.

fmt/clippy·Rust 전체476·실DB 초기 전체198(71.37초) 및 신규12개가 통과했다. 역할/미래 시각 검토를 반영한 최종 전체 실DB는199개/0실패(74.12초)로 확인했다. 원격 domain95/whole80·기존 전체8 CI는 PR의 최종 source에서 추가 확인한다. 문서262/Mermaid34/diff 검사를 실행했다. 실제 운영자 DB·키·매체·복구/RPO/RTO는 사용하거나 측정하지 않는다.
