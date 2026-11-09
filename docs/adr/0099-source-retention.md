# ADR0099: 만료 전적·비밀 자료의 물리 정리

- 상태: 채택 · 승인된 보존 설계의 구현
- M5 [#215](https://github.com/crystal23733/search-mine/issues/215), 상위 #25 · 선행 #213/PR214 병합·종료
- FR01/09/14/16 · NFR01/02/05/07/08 · TS15/16/17/18/20/21/28/29/31/35
- 근거: ADR0045/0067/0068, DB·인증/개인정보 설계. 정책 기간/개인 필드를 새로 늘리지 않는다.

## 결정

`PgSourceRetention`을 기존 retention port/maintenance에 연결한다. 별도 background 연결 풀을 만들지 않고 route5+maintenance2+detached owner1 한도를 유지한다. 한 batch의5단계는 각각100개 이하, 합계500개 이하의 원본 항목을 처리한다. 기존 maintenance와 같이 한 cycle은 최대8batch/2초이고60초마다 실행한다. `deleted`는 제거한 전적·definition 행과 비밀 seed/replay를 지운 원본 행 수의 합이다. 세부 사용자 정보/ID를 로그에 내보내지 않는다. 순간 물리 만료나 실제 장비 처리량을 보장했다고 보고하지 않는다.

1. 원래 `COALESCE(retention_started_at,recorded_at)`에서7776000초가 지난 online 결과/연결 player를 삭제한다. NULL fallback과 늦게 저장된 startup abort의 원래 anchor를 유지한다.
2. 원래 `record_expires_at` 경계에서 daily completion을 삭제한다.
3. `seed_expires_at` 경계에서 online `secret_seed`만 NULL로 지운다. 나머지 전적의 원래 기간을 다시 시작하지 않는다.
4. `replay_expires_at` 경계에서 daily `replay_json`만 NULL로 지운다. digest/불변 trigger/first-completion unique는 유지한다.
5. 검증 기간이 끝나고 completion의 참조가 없는 daily definition만 정리한다. 아직90일 전적이 참조하는 definition을 먼저 삭제하지 않는다. 만료 definition의 재게시/오래된 제출이 기간을 연장하지 않는 기존 계약을 유지한다.

각 단계는 `FOR UPDATE SKIP LOCKED`로 후보를 잠근다. 부모 source의 UPDATE 잠금은 FK 삽입의 KEY SHARE와 충돌하므로, 같은 source에 새 raw 삽입이 끼어드는 상태에서 max 시각을 읽고 삭제하지 않는다. source 삭제로 함께 지워질 essential raw 가운데 아직30일 미만인 원래 occurred_at이 있으면 **같은 transaction**에서 `coverage_since=max(기존값,max(영향 occurred_at)+1)`로 전진시킨다. singleton 누락·현재 Clock보다 미래인 raw·시각 상한 초과·권한/잠금/DB 오류면 source 삭제까지 rollback한다. 원래 collection 시작시각과 익명 보고서의13달력개월 expiry를 수정하지 않는다. 이미 불완전한 데이터를 완전한0으로 계산하지 않는 기존 metric reader 계약을 유지한다.

source FK cascade와 max(raw 시각) 조회가 전체 raw를 반복 scan하지 않도록 forward migration으로 두 source FK/occurred_at partial index, daily completion의 daily_id FK index, definition expiry/id index를 추가한다. 기존 migration/기간/보존 anchor는 바꾸지 않는다.

단조 Clock/지원 범위와2초 deadline을 SQL 단계·commit 전에 재검증한다. transaction-local statement/lock timeout과 외부 timeout을 같이 적용하고 pool로 timeout을 누출하지 않는다. source 삭제·account 삭제·원시 retention 경합은 실제 PG로 확인한다. serving 역할에 새 DDL/기간 수정 권한을 주지 않는다. 현재 grants에 이미 필요한 SELECT/UPDATE/DELETE가 있으므로 권한을 넓힐 필요가 없다.

## 재전송과 범위

실제 main은 `PgJournalRuntime`의 입장 journal을 요구한다. 정리된 final의 재전송은 active journal이 없으므로 새 result/secret을 생성하지 않는다. seed만 지운90일 이내 final은 duplicate 검증 후 NULL을 유지한다. daily는 만료된 publication/record를 다시 제출해 first completion을 재생성하지 않는다. 직접 저장용 과거 `PgResultRepository`는 입장 시각을 가지지 않는 legacy port이며 운영 main에 연결하지 않는다. 이를 서비스 복구/오래된 final 재주입 경로로 사용하지 않는다.

백업의 최신 삭제 기록·서비스 접근 차단·이전 release 호환과 실제 별도 매체/RPO/RTO는 #25 후속이다. 이번 이슈의 완료를 운영 복구 완료로 해석하지 않는다.

## 잠금 근거

[PostgreSQL row lock](https://www.postgresql.org/docs/current/explicit-locking.html)과 [SELECT SKIP LOCKED](https://www.postgresql.org/docs/current/sql-select.html)를 확인했다. SKIP LOCKED는 일관된 전체 조회의 대체가 아니라 maintenance 후보 경합을 건너뛰는 용도다. 완료/has_more 판단은 별도 조회로 남기고 실제 FK 삽입과 삭제의 경합을 검증한다.
