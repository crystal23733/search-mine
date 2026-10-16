# ADR0068 · 익명 집계 13개월 보존과 운영 조회

- 상태: 채택 · 승인된 FR01/16·상위20/하위145
- 선행: #143 CLOSED·PR144/develop1e58969 병합 확인
- 요구/시나리오: NFR01/02/05/07/08 · TS20/21/29/31/34/35/36

## 저장 내용과 원래 보존 기간

weekly_nsm의 UTC 월요일 또는 d7의 cohort UTC일을 원래 period로 삼는다. metric+period 키, computed_at/observed_through/complete와 닫힌 scalar counters, Rust aggregate_expires_at(period)의13달력개월 expiry만 저장한다. 임의 JSON payload·account/event/match/seed/명단/개인 mapping은 없다. 월말은 기존 Rust 달력 계약을 따른다. 재계산·수신시각은 원래 expiry를 연장하지 않고 metric/period/expiry 변경은 DB trigger가 차단한다. Rust typed model은 저장/조회 모두 시각·기간·counter 관계·nullable D7·완결성을 검사한다.

현재 계정삭제는 다음 RR 재계산에서 제외되며 이미 비식별화한 과거 집계는 개인 mapping 없이 원래13개월까지 유지한다. 같은 snapshot 도중의 삭제는 ADR0067의 RR 일관성을 따른다. 개인 권리는 기존 raw/source export·계정삭제로 유지하고 새 개인별 최초 날짜/평생 flag는 저장하지 않는다. 미래25의 원본 cleanup이 미만료 raw를 cascade하면 같은 transaction에서 coverage_since 전진을 연결해야 한다.

## 원자적 계산과 동시 저장

ADR0067의 snapshot 함수를 transaction 안에서 재사용한다. materialization은 READ WRITE REPEATABLE READ에서 권위 raw/source/계정 조회와 anonymous upsert를 함께 수행한다. pool/SQL/Clock/commit 총2초와 transaction-local 남은 statement_timeout, 마지막 IO 뒤 Clock/원래30일 window를 검사한다. 저장 조회도 pool 포함2초와 마지막 Clock/13개월 논리 만료를 검사한다. COMMIT 전 오류는 rollback하며 전송 뒤 응답 불명은 unavailable로 보고 다음 확인에 맡긴다.

Incomplete/Capacity/SQL/Clock/취소는 오류이고 과거 complete를0/false로 덮지 않는다. computed_at/observed_through는 역행하지 않고 complete는 후퇴하지 않는다. 같은 computed_at의 완전히 같은 scalar 보고서는 duplicate이며 다른 내용은 Conflict다. 초 단위 Clock에서 동시 snapshot의 오래된 결과가 최신 결과를 덮지 않게 다음 시각의 재계산을 요구한다. 동시 RR serialization 실패는 unavailable이며 다음 주기에 재확인한다. 현재 삭제 때문에 counters가 줄어드는 정상 재계산은 허용한다.

## 물리 정리와 자동 주기

13개월 논리 만료는 즉시 조회에서 제외한다. 실제 물리 삭제는 원래 expiry 인덱스의 최대500개·FOR UPDATE SKIP LOCKED·남은2초를 사용하고 기존 retention port의 최대8batch/총2초/60초·즉시tick/skip/진전0 중단을 재사용한다. raw30일 worker와 anonymous13개월 worker는 별도로 실행해 한쪽 집계/정리 실패가 다른 정리를 멈추지 않는다. 잠금·중단·backlog 동안 물리 삭제가 지연될 수 있으며 실운영 처리량은25/26에서 측정한다. 로그는 component/code/count만 쓴다.

main은 owner recovery와 collection 초기화 뒤 별도 aggregate maintenance를 실행한다. 즉시 첫 tick/60초·missed tick skip·겹침0, 현재 주 NSM과 순환 대상 하나(이전 주 또는 today-8..today-29 UTC일의 mature D7)를 계산한다. 최대2report/cycle·각2초/총4초다. 특정 D7/이전 주 갱신은 정상 상태 약23분 주기이며 원래 collection/30일 window 밖이나 old gap은 Incomplete로 남는다. current NSM 실패도 순환 대상 계산을 막지 않는다. 유효 Clock으로 고른 period만 사용하며 오류를0 집계로 보고하지 않는다. 누락/만료/오래된 값과 실제 computed_at/observed_through를 운영 조회에 표시한다.

세 maintenance는 요청 pool의 연결을 점유하지 않도록 같은 DB 역할의 별도 pool(max2/acquire1초)을 공유한다. 요청 pool max5와 기존 detached journal owner1을 합쳐 프로세스당 최대8연결이며 SQL 부하 자체는26의 실측 출구다. refresh port는0보다 크고2초 이하인 남은 allowance를 받아 DB statement_timeout까지 전달한다. cycle은 두 report에 남은4초를 각각 최대2초로 전달한다. 실패는 다음60초 tick에서 재확인하며 raw/aggregate 정리와 materialization의 오류를 서로 빈 성공으로 바꾸지 않는다.

## 운영 조회와 권한

로컬 metrics CLI는 닫힌 metric/UTC날짜와 read 또는 명시 refresh만 받는다. DATABASE_URL은 env로만 읽고 SQL/접속 문자열/ID를 오류에 출력하지 않는다. JSON에는 기간/표본/nullable D7 rate/complete/계산시각/원래expiry/stale를 표시한다. 없으면 missing,13개월 밖은 expired이며0으로 가장하지 않는다. stale는30분을 기준으로 실제 계산시각과 비교하고 과거 final 보고서도 계산시각을 숨기지 않는다. refresh는 collection 시작을 초기화하지 않는다. 공개 HTTP admin/외부 SaaS/메시지 전송은 추가하지 않는다.

forward migration은 별도 migration 역할로 적용한다. serving에는 aggregate SELECT/INSERT/DELETE와 upsert 대상 computed/observed/complete/counter column UPDATE만 부여한다. SELECT FOR UPDATE의 row-lock 권한도 이 최소 UPDATE로 검증하고 metric/period/expiry UPDATE·DDL/migration 권한은 부여하지 않는다. 불변/단조 trigger를 유지한다.

## 인수 검증

실제 PG Red→Green: 익명 필드만 저장·월말/exact13개월·원래expiry 불변·현재삭제 재계산/과거anonymous 유지·Incomplete/Capacity 비덮어쓰기·동시/equalClock·SQL/Clock/취소 rollback·최소DML·물리500+1/잠긴row/동시cleanup·실제CLI/main을 확인한다. maintenance는60초/23분순환/4초공유budget/실패독립/겹침0·stale를 검사한다. full/실DB/freshdomain95전체80·docs/Mermaid·실제 코드리뷰·최신6CI/정확head squash merge/CLOSED와 상위20 인수 trace audit를 기록한다. 고정 fixture Clock이 이전 실행 기록과 섞이지 않게 전체 E2E마다 독립 검증 DB를 만들며 기존 DB/logs/trace를 보존한다. optional21·OAuth42·운영25/26·사람27·미확정83/141은 별도이며 실사용 지표/운영 성능을 달성했다고 주장하지 않는다.
