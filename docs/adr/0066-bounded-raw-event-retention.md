# ADR0066 · 원시 이벤트 30일 물리 보존

- 상태: 채택 · 승인된 FR01/16·상위20/하위140
- 선행: #138 CLOSED·PR139/develop3d53a20 병합 확인
- 요구/시나리오: NFR01/02/05/07/08 · TS20/21/29/31/35

## 원래 시각과 제한된 삭제

원시 필수 이벤트는 Rust RAW_EVENT_SECONDS의 원래 occurred_at에서30일이 되는 순간부터 조회·재수집에서 제외한다. 기존 논리 만료에 실제 PostgreSQL DELETE를 연결한다. received_at·재시도·worker 실행 시각으로 보존 기간을 연장하지 않는다. 원본 게임90일·replay7일과 account 삭제 정책은 기존 기준이다. event 삭제로 원본 게임을 삭제하거나 재전송으로 raw를 복원하지 않는다.

한 transaction은 기존 occurred_at 인덱스로 오래된 만료 row부터 최대500개를 FOR UPDATE SKIP LOCKED로 선택해 삭제한다. 같은 시각 row의 ID 순서는 삭제 의미에 영향을 주지 않는다. 새 DDL이나 과거 migration 변경 없이 기존 인덱스를 사용한다. 전체 pool/SQL/commit은2초이며 시작·pool 획득 뒤·삭제 뒤·commit 전 Clock의 유효 범위와 역행을 검사한다. commit 전 오류·취소·시간 초과는 rollback하고 빈 성공으로 바꾸지 않는다. COMMIT 전송 뒤 응답 유실/취소는 이미 commit됐을 수도 있으므로 unavailable로 보고하며 다음 idempotent 주기에 다시 확인한다. 반환은 삭제 count와 조회 snapshot의 만료 backlog boolean뿐이며 row/account/source ID를 반환하지 않는다. 다른 worker와 동시 실행할 때 backlog는 관측 시점의 힌트다.

삭제 중 account/원본을 잠그지 않는다. account/source cascade와 직렬화되는 row lock만 사용하고 잠긴 row는 다음 주기에 처리한다. 조회의 논리 만료는 물리 worker/잠금/장애와 독립적으로 즉시 적용된다. 물리 주기는 정상 상태60초이며 backlog·잠금·DB 장애 때 지연될 수 있어 즉시 모든 물리 삭제를 보장한다고 보고하지 않는다. 운영 환경 처리량/디스크/최대 지연 실측은26/25의 별도 출구다.

## 작업 조립과 권한

RetentionStore port와 PostgreSQL adapter를 분리한다. main은 인증 DB와 journal owner를 준비한 뒤 별도 async maintenance를 시작한다. 즉시 첫 tick·60초 간격·missed tick skip, 한 cycle 최대8batch·총2초, 완료 또는 진전0이면 중단한다. 게임 요청이 maintenance 완료를 기다리지 않는다. 실패는 다음 주기에 재시도하며 stable component/code/count만 내부 로그에 남긴다. 부분 batch가 이미 commit됐어도 cycle timeout을 삭제0 성공으로 보고하지 않는다. shutdown은 기존 runtime 수명을 따른다.

port는0보다 크고2초 이하인 남은 budget만 받는다. cycle은 각 batch에 남은 시간을 전달하며 adapter는 같은 deadline의 client timeout과 transaction-local PostgreSQL statement_timeout을 함께 적용한다. pool 대기로 소모한 시간을 SQL에 다시 부여하지 않고 후속 SELECT/COMMIT 전 남은 budget을 갱신한다. 취소 후 DB 쿼리가 무제한 남지 않도록 한다.

serving writer의 event SELECT/INSERT/DELETE 권한과 달리 FOR UPDATE를 쓰는 cleanup의 최소 row-lock 권한은 실제 제한 역할로 검증한다. 필요한 경우 product_events UPDATE(id)만 추가한다. 불변 trigger는 ID 변경·발생/수신시각 변경을 계속 차단하며 다른 column UPDATE·DDL/migration 권한은 부여하지 않는다. 공개 cleanup endpoint·클라이언트 수집·외부 SaaS는 만들지 않는다.

## 검증과 다음 출구

실제 PG Red→Green으로 정확30일 전후·수신시각 비연장·500+1/동시 worker·잠긴 row/backlog·SQL 실패/Clock 역행 rollback·pool/SQL2초·본인 논리 조회와 실제 삭제·source retry 비복원을 확인한다. 실제 main 시작에서 만료 raw만 사라지고 게임/권리 경로는 계속 사용 가능해야 한다. worker port 시험은8batch·2초·진전0·tick skip/겹침을 확인한다. full/실DB/95-80·docs/Mermaid·코드리뷰·필수6CI/정확head 병합/CLOSED를 확인한다.

익명 집계 테이블은 아직 없다. calendar13개월의 실제 저장/물리 만료는 다음 first-normal/NSM·D7/운영조회 하위에서 이 maintenance에 연결한다. 이 원시 출구만으로20/M3·실제 CMP/사용자지표를 종료하지 않는다. 외부42·25/26·27·미확정83/141은 별도다.
