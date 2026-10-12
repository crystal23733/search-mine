# ADR0060 · 데일리 client와 명시적 대기 제출

- 상태: 채택 · M3/#128, 선행 #126/PR127 병합·CLOSED 확인
- 요구: FR01/09/10/16, NFR01/02/05/07/08, TS17/18/20/21/29/31/35/36

## 결정

Rust 생성 DTO의 start/heartbeat/definition/submit-timed/own/rankings를 typed client port로 제공한다. 고정 same-origin POST와 closed JSON·UTF-8·입출력 byte 상한·abort/10초 기한을 적용한다. RulesSnapshot 구조 decoder는 기존 온라인 decoder를 재사용한다. hash/seed/규칙 의미와 공식 시간 판정은 Rust 서버/코어만 수행한다.

인증 요청은 기존 AuthenticatedRequests.execute의 전후 fresh proof·현재 account/monotonic revision을 유지한다. RequestContext에 opaque server sessionRevision을 필수 메모리 값으로 전달한다. cookie/CSRF/revision/account를 저장소에 추가하지 않는다. 응답의 account/attempt/ticket과 전체 metadata를 결합 검증한다. Practice는 원래 첫 receipt의 attempt가 다를 수 있다. nullable verified_elapsed_ms와 trust를 함께 검사하고 client 시간을 공식 시간으로 승격하지 않는다.

공개 순위는 credentials:omit로 인증/CSRF 없이 조회하며 정상 미발행/빈 목록과 실패를 구분한다. 요청 동시2개·10초·응답64KiB, 서버의 각50개·ordinal·has_more 계약을 검사한다. 내부 ID와 추가 필드는 거절한다.

실제 설정에는 아직 제출 버튼이 없고 main transport도 placeholder였다. 현재 계정 닉네임을 표시하는 명시 확인 창과 제출 진입점을 추가한다. 계정/revision/연결·activity guard를 확인하며 중복 클릭을 막는다. 로그인/복구만으로 자동 flush하지 않는다. 각 후보의 accepted definition 전체 metadata를 먼저 확인하고 ticket_id:null로 제출한다. 유효한 metadata 불일치만 unsupported로, 정확한 HTTP410/daily_expired만 expired로 분류한다. 이 종료 결과도 execute callback 안에서 정상 값으로 반환하여 반드시 post-proof 후 확정한다. 일반400·손상·timeout·네트워크·계정 전환·늦은 응답은 후보를 보존한다. accepted/expired/unsupported만 pending에서 제거하고 개인 기록은 유지한다. 기존 attempt UUID와 서버 idempotency를 사용한다.

## 검증과 후속

실제 설정의 누락된 제출 진입점을 HTTPS 행동 Red로 확인한 뒤 실제 WASM 완료 후보/8언어/PC·mobile·현재 계정 확인/PG·own/재제출·개인 기록 보존을 검사한다. fake fetch/stream/clock/session으로 strict DTO·상한·취소·계정 변경과 종료 결과의 post-proof를 검사한다. 명시적 공식 시작/controller/heartbeat·순위 화면과 상위 #19 전체는 다음 하위다. OAuth #42, 운영 #25/26, 사람 #27, 원인 미확정 #83은 별도다.

공통 wire/RulesSnapshot decoder와 bounded JSON reader를 추출해 온라인/daily가 재사용한다. 기존 온라인 오류 adapter의 코드/기본128KiB 계약은 유지하고 daily는64KiB를 명시한다.

## PR129 응답 관측 보정

최초 CI38273165548의 모바일 명시 제출 시험은 pending0/개인기록1 확인 뒤 브라우저 Response.json의 Network.getResponseBody:No data found로 실패하고 재시도됐다. 실제 navigation이나 제품 제출 실패로 단정하지 않는다. [ADR0043](0043-result-response-observation.md)의 실제 HTTP 전달 전 관측을 제출 경계에도 적용한다. 최종 성공 요청만 one-use route.fetch(maxRedirects0/maxRetries0/10000ms)로 전달해 실제200/no-store 본문을 읽고 같은 APIResponse를 fulfill한다. 클릭과 관측 promise를 함께 기다리고 fetch/body/fulfill 오류는 reject·abort로 드러낸다. 기존503 보존/명시 계정 확인·8언어·PC/mobile·DB 첫 완료/Practice/개인 기록 단언, 제품 코드와 timeout/retry는 유지한다. 최초 실패 artifact와 수정 후 재시도0 반복·전체 실행을 구분해 기록한다.
