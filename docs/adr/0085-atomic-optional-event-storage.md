# ADR0085 · 현재 동의의 원자적 선택 이벤트 저장

- 상태: 채택 · 기존 승인 FR01/13/16·NFR01/02/05/07/08 · #184 / 상위21
- 선행: #174·#178·#182/PR183 병합·CLOSED, develop5817532
- 시나리오: TS20/21/22/24/29/31/35/36. 실제 CMP·공개 collector와 독립적인 내부 저장 adapter다.

## 현재 서버 권위와 탭 상관키

OptionalEventStore port는 기존 검증된 ClientDraft, 서버가 도출한 ConsentSession(account/current session revision/token hash), 기대 동의 revision을 받는다. grant boolean/브라우저 snapshot/TC 문자열을 저장 권위로 받지 않는다. PostgreSQL adapter는 account→session 순서 잠금과 현재 token/account/session·생성 시각0 이상/인증 시각 순서·시계/기한 검증 뒤 같은 transaction에서 session_consents 현재 행을 읽는다. subject가 현재 auth session ID이고 revision이 기대값과 같으며 목적·발생 시각·현재 기한을 충족할 때만 저장한다. 저장 직전에도 같은 잠금 아래 현재 동의·세션·시각을 다시 검사한다. 전체2초·try4동시·기존 pool만 사용한다.

요청 future의 timeout만으로 실제 SQL 대기가 끝나지 않는 것을 실PG Red로 확인했다. 선택 저장은 기존2초 Instant deadline에서 pool/BEGIN 소모분을 뺀 남은 시간을 transaction-local `transaction_timeout`으로 한 번 설정하고 같은 client deadline을 유지한다. [PostgreSQL18 공식 설명](https://www.postgresql.org/docs/18/runtime-config-client.html#GUC-TRANSACTION-TIMEOUT)에 따른 transaction 전체 기한이며 전역 설정을 바꾸지 않는다. 만료 시 해당 세션이 종료되어 미commit 기록/잠금이 해제되고 pool이 연결을 교체한다. timeout 뒤 blocker를 풀기 전의 실제 backend 종료·pool 대기 예산 비연장·정상 요청/기본 설정 복귀를 검증한다. caller 취소도 원래 backend 기한을 없애지 않는다.

ADR0063의 draft.session_key는 탭 내 일시 opaque 상관키이며 auth session ID/토큰과 다르다. 서버 권위는 별도 검증한 ConsentSession/CurrentConsent에서만 온다. 현재 허용을 기존 admit_client 목적/기간 정책으로 투영할 때 snapshot subject는 그 draft의 탭 상관키로 지정한다. 이 투영은 내부 저장 경계에서만 수행하고 client가 작성한 snapshot을 받지 않는다. 인증 세션 ID를 탭 key로 복사하거나 공개하지 않는다. 계정/세션의 동의가 없는 다른 tabkey의 존재 자체는 허용을 만들지 못한다. 익명 subject·무계정 서버 계정은 만들지 않는다.

## 저장과 재시도

기존 product_events의 essential-only CHECK와 필수 NSM/D7·정산/승패 권위를 유지한다. 별도 optional_product_events는 event_id PK, 서버 owner_session_id FK, opaque tab_key, 삽입 당시 consent_revision, analytics/advertising purpose, occurred_at/received_at, 8locale, 닫힌 ClientAction JSON만 저장한다. 임의 JSON/URL·추가 개인정보/TC·비밀값은 없다. UUID·목적/type·시각/30일 미만·locale·object/1024byte 제약과 UPDATE 불변 trigger를 둔다. 목적은 Rust ClientAction에서 도출하고 기존 decoder/domain을 재사용한다.

같은 ID+같은 전체 draft+같은 서버 owner의 재시도는 한 행·원래 received_at/증거 revision을 유지한다. 같은 ID의 다른 내용/owner는 conflict다. revision은 현재 권한 조건이며 중복 확인보다 앞서 검사한다. 철회·실패·만료 뒤 duplicate도 거절한다. 현재 regrant가 있더라도 발생 뒤 동의는 허용되지 않는다. 동의 갱신으로 기존 이벤트를 수정하지 않는다. 재시도에서 저장된 receipt가 미래이면 실패하며 원래 발생 시각의30일 보존을 연장하지 않는다.

같은 account→session 잠금 때문에 철회/refresh/회전/logout/계정 삭제와 저장의 commit이 직렬화된다. 먼저 완료된 철회/회전/삭제 뒤에는 저장하지 못하고 먼저 허용 저장이 commit된 뒤 철회가 완료되면 이후 요청은 거절한다. logout/회전·계정 삭제는 owner session FK로 선택 raw를 cascade한다. caller 취소/2초 timeout은 미commit transaction을 rollback하며 실패를 허용으로 바꾸지 않는다. committed receipt의 유실은 같은 ID 재시도로 확인한다.

## 보존과 실행 조립

원래 occurred_at에서 정확30×86400초에 논리 거절한다. 별도 PgOptionalEventRetention은 기존 EventRetentionStore/Maintenance 계약의500행/SKIP LOCKED·각 cycle2초/최대8batch·60초 주기·transaction-local statement budget을 재사용한다. main의 기존 maintenance pool로 정리 task만 연결하고 새 pool을 만들지 않는다. 필수/익명 집계 보존을 변경하지 않는다. 제한 역할에 optional table SELECT/INSERT/DELETE와 raw 두 테이블의 행 잠금용 UPDATE(id), 기존 auth/consent 읽기·잠금에 필요한 권한만 부여하는 실PG 검사를 둔다. [ADR0066](0066-bounded-raw-event-retention.md)의 기존 기준처럼 FOR UPDATE SKIP LOCKED에는 최소 한 열의 UPDATE 권한이 필요하므로 id만 허용하고 payload UPDATE/DDL은 부여하지 않는다. id 자체의 변경도 기존 불변 trigger가 거절한다. 필수 raw 보존에도 같은 잠금 권한이 필요함을 제한 역할 검사로 확인한다. 운영 역할 적용은 기존 별도 migration/운영 절차를 따른다.

이번에는 공개 선택 수집 HTTP·client collector·SDK·실제 CMP verifier·익명 subject를 추가하지 않는다. main/HTTPS fixture는 DisabledConsentVerifier를 유지하고 실제 grant를 만들지 않는다. 내부 adapter의 테스트용 verifier와 raw fixture는 공개 제품 경로가 아니다. 현재 조회를 수집 권한으로 사용하는 구현도 추가하지 않는다.

## 검증

문서/시나리오→행동 Red→Green 순으로 목적·미확인/거부/만료·기대 revision·별도 tabkey/서버 subject·발생 뒤 grant·시각/30일을 검사한다. 실제PG는 동시 동일ID/변경 충돌·철회/재검증/CAS·다른 계정/session/token·lock 대기 중 만료/역행/timeout·취소·logout/회전/삭제 cascade·bounded 물리 보존·제한 DML/불변을 확인한다. 새 pure optional 모델도 Event domain95% gate에 포함하며 server80% 기준을 유지한다. 실제 CMP/운영147·상위21 전체·83 해결·정산/실사용 지표 달성은 미완료다.
