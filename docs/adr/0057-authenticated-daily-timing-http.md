# ADR0057: 인증 데일리 시작·하트비트·시간 제출 HTTP

- 상태: 채택 (승인된 #19 순차 구현)
- 일자: 2026-10-12
- 연결: #122, 선행 #120/PR121 develop08eede7 병합·CLOSED 확인
- 요구: FR01/09/10/16, NFR01/02/05/07/08, TS17/18/20/21/31/35/36

## 결정과 계약

[ADR0055](0055-bounded-daily-time-authority.md)의 ticket 권위와 [ADR0056](0056-daily-time-persistence-and-export.md)의 sealed time PG 저장을 고정 POST `/api/v1/daily/start`, `/heartbeat`, `/submit-timed`에 연결한다. 기존 definition/submit/own과 같은 context·requests4·rate20/초·전체HTTP2초·CPU2/2초를 공유한다. Rust closed DTO→생성TS이며 알 수 없는 필드·중복 필드·query·잘못된 canonical UUID/version을 거절한다. 현재 account/session revision/nickname/Origin/CSRF 및 응답 직전 같은 실제 session proof/철회 select/barrier를 적용한다.

start 입력은 v/account_id/session_revision/attempt_id뿐이다. 날짜·seed·rules·client elapsed를 받지 않는다. 주입 AuthClock의 현재 UTC 날짜와 seed version1으로 accepted snapshot을 조회/게시하고 시작 직전 날짜를 재확인한다. UTC 자정 사이에 준비되면 conflict로 재시도한다. chrono의 UTC timestamp 변환을 사용하며 달력 알고리즘을 복제하지 않는다. 응답은 v/account/attempt/ticket UUID·accepted metadata/rules·원래 ttl_ms·heartbeat_interval_ms다. 같은 attempt 재전송은 원래 ticket/TTL이며 시작 시각을 연장하지 않는다.

heartbeat 입력은 v/account/session revision/ticket_id/sequence다. 응답은 v/account/ticket 및 nullable ack(sequence,elapsed_ms)다. ack null은 시간 자격이 없다는 뜻이다. 15초 권장, 45초 이상 gap은 영구 강등, 연속 순번1부터 마지막 동일 순번 재전송은 원래 ACK/시각이다. 잘못된 순번은 conflict/invalid이며 시간 자격을 조용히 복구하지 않는다.

submit-timed는 별도 closed DTO(v/account/session revision/attempt_id/ticket_id/replay)다. ticket_id는 필수 nullable이며 null은 오프라인·소실 제출이다. 실제 accepted Native ReplayEvidence를 검증한 뒤 같은 lease/revision/ticket/attempt로 finish한다. 유효 sealed proof만 submit_timed 저장 포트에 전달한다. missing/expired/restart/45초 gap/너무 짧은 server elapsed는 plain submit으로 완료만 기록한다. metadata/sequence/권한/clock 오류는 fallback하지 않는다. 응답은 기존 DailySubmitResponse이며 trust는 저장된 첫 receipt의 nullable 시간만 따른다. NULL 사후 승격과 원래 retry/Practice 시간 개선은 금지한다.

## 조립·수명·운영

DailyAuthentication은 ticket registry와 AuthClock을 받는 생성자로 같은 AuthorityRegistry를 유도한다. 서로 다른 authority를 조립할 공개 struct 필드를 없앤다. 모든 daily 요청에서 generation/read/bind 전에 reap을 실행해 만료 ticket의 마지막 lease가 새 계정 admission을 막지 않게 한다. start/heartbeat/finish의 짧은 memory commit 외에 IO/async/native 작업은 authority/ticket 잠금 밖에서 실행한다. 단조 clock 역행은 registry 전체를 영구 fail closed로 만든다.

main과 HTTPS fixture는 같은 registry를 HTTP와 CombinedSessionInvalidator의 기존 daily child에 주입한다. registry가 authority 철회와 실제 barrier를 반환하므로 authority child를 중복 등록하지 않는다. LIAR_DAILY_TICKETS는 기본64, 엄격 범위1..20000이다. auth 비활성 시 새 세 경로도503/no-store다. 기존 migration과 개인정보 최소 수집·raw7일/record90일·계정 cascade를 유지한다.

## 검증과 후속

실제 HTTP Red→Green 후 current UTC/accepted snapshot·closed 입력·heartbeat 원래 ACK/45초/TTL·sealed time/NULL fallback/원래 retry·공유 rate/capacity/timeout·reap-before-bind·현재 proof와 철회/삭제·실제PG/native/HTTPS를 검사한다. full/fresh domain95/server80/문서/Mermaid/코드리뷰·최신CI/exact-head 병합·122CLOSED를 완료한다. 주입 fixture clock은 실제 인간 풀이·벽시계 지연·사전 계산 방지·글로벌 RTT 성능의 근거가 아니다. 두 순위·8언어 명시 공식 시작UI/owned pending·상위19와 외부42·물리25·하드웨어26·사람27·미확정SW83은 후속이다.
