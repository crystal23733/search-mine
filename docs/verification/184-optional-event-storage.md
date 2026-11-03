# #184 · 현재 동의의 선택 이벤트 저장·보존

[ADR0085](../adr/0085-atomic-optional-event-storage.md). FR01/13/16·NFR01/02/05/07/08·TS20/21/22/24/29/31/35/36.

선행174/178/182 병합·CLOSED와 PR183 squash5817532·최신 develop clean·원격0/0을 확인했다. 설계·시나리오를 제품 코드 전에 기록하고 feature/184-optional-event-storage에서 구현했다.

## 변경과 실제 코드 리뷰

OptionalEventStore port는 서버 ConsentSession·기대 동의 revision과 기존 검증된 ClientDraft를 받는다. adapter가 account→session 잠금 아래 token/account/현재 auth session과 session_consents를 읽고 목적·발생/현재 시각·revision을 검증한다. 저장 뒤 commit 전에도 같은 잠금 아래 세션·현재 동의·시계를 재확인한다. pool 대기 시간을 뺀 원래 Instant deadline을 transaction-local PostgreSQL transaction_timeout에도 적용해 요청 종료 뒤 실제 SQL 대기가 무제한 남지 않게 한다. 전체2초/try4동시·기존 pool이며 verifier IO나 공개 collector를 추가하지 않는다.

현재 서버 동의의 subject와 auth session ID가 같은지 먼저 확인한 뒤 별도 탭 상관키로 기존 admit_client 정책을 투영한다. auth session ID/토큰을 tab session_key로 복사하지 않는다. 내부 투영은 client snapshot이 아니고 기대 revision은 현재 권한 조건이다. 기존 Consent의 account/session SQL 검증은 공통 transaction helper로 그대로 추출했고, 목적/기간 규칙은 Rust 기존 도메인을 재사용했다. 새 pure optional 모델도 Event domain95% 그룹에 포함했다.

별도 optional_product_events에 최소 owner session FK·탭 상관키·삽입 당시 동의 revision·목적·발생/수신 시각·locale·닫힌 ClientAction만 저장한다. UUID/time/30일 미만·8locale·목적/type·object/1024byte/닫힌 key 제약과 기존 불변 trigger를 사용한다. 같은 ID/내용/서버 owner는 한 행·원래 receipt/proof를 유지하고 다른 내용/owner는 충돌한다. duplicate도 먼저 현재 권한을 검사하며 철회 뒤·발생 뒤 재동의·미래 receipt/시계 역행은 거절한다. logout/회전/계정 삭제는 session FK로 선택 raw를 cascade한다. 필수 product_events의 essential-only·NSM/D7·정산 권위는 유지한다.

보존은 기존 로직을 닫힌 RawEventTable 선택으로 공유해 SQL/시계/남은 deadline을 중복하지 않는다. 테이블 선택은 두 내부 상수뿐이고 데이터는 바인딩한다. 정확30일·500행/SKIP LOCKED·cycle2초/최대8batch·60초/진전0 중단을 재사용하며 main의 기존 maintenance pool로 정리 task만 추가했다. 실제 main의 component/삭제 count와 활성 세션을 검사했다. 관측 로그는 닫힌 component/code/count만 유지한다.

새 의존성/공개 wire·추가 개인정보·동의 이력·파일 다운로드가 없다. migration은 선택 raw 테이블 추가이며 기존 migration/원본/게임 기록을 변경하지 않는다. main/HTTPS fixture의 DisabledConsentVerifier와 선택 수집 차단은 유지한다. 실제 CMP·공개 collector/client 이벤트·익명 subject·운영147·상위21 전체·83 해결은 후속이다. 물리 정리 backlog/장애/운영 처리량은 기존25/26 검수와 구분한다.

## Red → Green과 보완

실제 stub 실행에서 pure admission2개·선택 PG port2개·보존 no-op2개·main 조립1개가 행동 실패했고 구현 뒤 통과했다. 최종 리뷰에서 DB 세션의 음수 생성 시각을 내부 공유 helper가 거절하지 않는 것을 실PG Red1로 재현했고, 기존 Auth Session.current_at과 같은 하한 검사를 추가했다. 컴파일/준비 오류(UUID 직렬화·private field 접근·sqlx0.9 JSON 기능/동적 SQL 문자열)는 Red로 계산하지 않는다. 새 feature/dependency 없이 canonical 문자열·바인딩 text::jsonb·QueryBuilder로 해결했다.

제한 DML fixture는 현재 동의 조회/저장까지 통과하고 cleanup row lock에서 실패했다. 기존 [ADR0066](../adr/0066-bounded-raw-event-retention.md)의 UPDATE(id) 최소 열 권한을 새 fixture에서 누락한 것이었다. 이를 적용해 선택/필수 raw 정리가 모두 통과했고 payload·실제 id 변경·동의 발급·DDL·migration 조회는 계속 거절했다. auth 테이블도 SELECT/UPDATE(id)만으로 port의 잠금을 검증했다. README와 과거 #138 설명을 현재 #140 기준과 일치시켰다. 운영 DB 권한을 실제로 변경하지 않았다.

## 실제 로컬 실행

| 검사 | 결과 |
|---|---|
| Rust workspace | 최종453 passed, 실DB174 ignored (별도 실행) |
| PostgreSQL18 전체 | 최종174 passed / 20.33초; 새14개 포함 |
| 목적/저장/권리10개 | grant 목적 분리·동시 동일ID/receipt 불변·stale/unknown/foreign·철회와 저장 두 잠금 순서·insert 대기 중 만료/역행/2초/취소·logout/회전/erase cascade·재동의/DB 제약·실제4대기/5th거절·제한 DML·다른 요청의 receipt 시계 역행 |
| 선택 보존4개 | 정확30일·500+1/잠긴 행·기한/취소/역행 rollback·실제 main 즉시 정리/활성 세션 보존 |
| 기존 보존 회귀 | 공유 helper 도입 뒤17 passed / 2.83초; 최종 전체174에도 포함 |
| 최소 권한 최종 | 제한 DML1 passed / 0.38초; auth/raw의 id 열 잠금만, payload/DDL/동의 발급 거절 |
| 정적/생성 | fmt·clippy workspace/all-targets·WASM check·생성 TS freshness 통과 |
| 문서 | 229 Markdown·제품21·설계18·시나리오36, Mermaid35/오류0 |

소스 변경은 Rust/SQL/문서이며 이번 이슈의 로컬 웹 단위/전체 browser·PowerShell check.ps1 자체·LLVM coverage는 미실행이다. 정확 head의 CI로 전체 web/browser·기존 domain95/server80을 확인하고 실제 결과를 PR에 기록한다. 실제 CMP/운영/사람 검수를 대신하지 않는다. 로컬 Red/SQL·실제 main 진단과 검증 로그는 ignored .tmp/optional184-* 및 review/YYYY-MM-DD.md에 보존한다. 코드 리뷰·필수 CI·최신 head/base·PR 병합/이슈 종료는 실제 수행 결과를 PR에 기록한다.


## 최초 CI와 실제 backend 기한 보완

source e2ff7f8 / Product41633828173의 필수6종이 성공했다. Rust453/173ignored·실PG173(70.51초)/계측174(70.83초), Event509/510=99.80%·Consent167/167=100%·서버11997/12505=95.94%·core2338/2399=97.46%가 기존 기준을 통과했다. 웹 단위372, 전체 browser125 passed/1 flaky/10.0분이다. 기존83의 offline-mobile224행 load10000ms가 첫 시도 실패 후 retry에서 통과했고 trace1·로그/layout을 .tmp/optional184-first-ci-*에 보존했다. 이를83 해결이나 재시도 없는 성공으로 표현하지 않는다.

이 green만으로 병합하지 않고 실제 backend 수명 검증을 추가했다. 요청이2초에 반환돼도 blocker를 해제하지 않으면 SQL 대기가 남는 것을 실PG Red1로 재현했다. 선택 transaction에만 남은 원래 기한의 transaction_timeout을 설정하고 같은 client timeout_at을 유지했다. 전역 DB 설정을 변경하지 않는다. 공식 PostgreSQL18 근거는 ADR0085에 연결했다. blocker 미해제 상태의 backend 종료·caller취소 뒤 원래기한 종료·750ms pool대기 예산 비연장·새 연결/정상 수집/기본설정0복귀·기존300ms기한비연장을 확인했다. 관련10개가4.90초에 통과했다.

추가 pool 검사는 반환할 연결을 보유한 채 pool.close를 기다리는 fixture 종료 순서 오류로 처음 대기했다. 실제 DB에는 대기 SQL 없이 idle 연결만 남았음을 확인하고 해당 로컬 시험 프로세스2개만 종료한 뒤 연결을 먼저 반환하도록 고쳤다. 이 준비 오류나 기존 짧은 기한 검사를 행동 Red로 계산하지 않는다. 최종 전체/계측/CI는 이 보완을 포함한 새 source로 다시 확인하며 최초 CI와 구분한다.
