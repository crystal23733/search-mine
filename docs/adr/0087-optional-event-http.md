# ADR0087 · 현재 세션의 선택 이벤트 수집 HTTP

- 상태: 채택 · #188 / 상위21 · FR01/13/16·NFR01/02/05/07/08
- 선행: #184/PR185·#186/PR187 병합·CLOSED / develop8b1d1d6
- 시나리오: TS20/21/22/24/29/31/35/36

## 계약과 목적

사용자가 서비스 개선/광고 전달 통계의 목적을 확인한 뒤 진행을 지시했다. 기존 [ADR0085](0085-atomic-optional-event-storage.md)의 선택 저장을 HTTP에 연결한다. POST /api/v1/events/optional은 v1·expected_revision·draft 단건만 받는다. draft는 기존 event_version/event_id/session_key/occurred_at/locale/event 계약이고 event별 임의 필드를 거절한다. 기존 ClientAction과 draft wire 구조를 Rust protocol의 generic ID 계약으로 추출해 서버의 검증된 EventId와 클라이언트 string에 공유한다. TS는 Rust에서 생성하며 검증 규칙을 TS에 복제하지 않는다. 목적은 event 종류로 도출하고 client subject/account·grant·서버fact·URL·개인정보를 받지 않는다.

전체 입력2048byte·draft1024byte, v1, canonical nonnil expected revision과 EventId,8locale·시각/일자/소요시간 규칙을 유지한다. 같은 event_id의 receipt는 v/event_id/원래received_at만 반환한다. inserted 여부·소유자·payload·원시기록은 공개하지 않는다. 같은 ID 재시도 성공은 항상200, 다른내용/owner 충돌409, 동의거부/만료403, 인증없음401, CSRF/Origin403, 형식400/크기413, 속도429, 용량/기한/저장장애503이다. 실패도 no-store/no-referrer/nosniff이며 응답에는 닫힌 error만 보낸다.

## 인증·자원·취소

현재 cookie/Origin/CSRF 검증·account nickname 없는 현재 세션도 허용한다. 세션/계정ID와 token hash는 서버가 선택한다. 전후 bounded SessionReader 확인·AuthorityRegistry의 공유 guard/취소·clock 역행/만료 검사를 재사용한다. 같은 실제 권리 레지스트리를 main/HTTPS의 계정권리와 공유한다. 세션과 계정별 초당20/추적4096·HTTP동시4, DB동시4, 본문 수집 바깥에서 원래2초를 한 번 생성한다. 공개 수집 실패가 게임 명령을 차단하거나 백그라운드 재전송 queue를 만들지 않는다.

OptionalEventStore에 absolute std Instant를 받는 capture_until을 추가하고 기존 capture는 기본2초로 위임한다. HTTP 본문/인증/전후 session·pool/저장/실제 backend에 원래 기한을 전달한다. [ADR0086](0086-consent-operation-deadline.md)의 PG transaction-local helper를 사용한다. 만료/2초초과 기한은 DB 작업 전에 거절하며 immutable 요청값만 사용한다. 현재 subject/revision·목적/발생시각과 권리 검사는 저장 transaction 안에서 수행하고 철회/회전/logout/삭제와 직렬화한다. commit 뒤 권한변경/응답유실은 성공으로 위장하지 않고 같은ID의 현재권한 아래 재시도로 확인한다. 거부 뒤 duplicate도 저장권한을 우회하지 못한다.

## 조립과 검증

main/실HTTPS fixture에 API를 조립한다. DB/인증 미설정 실행은 같은 경로503이며 실제 verifier는 계속 Disabled다. 테스트의 grant verifier는 격리된 검증에만 주입하고 실행앱에 거짓grant를 넣지 않는다. 브라우저 자동전송·CMP/SDK·광고·익명 subject·전체정책/실운영147·상위21/83 완료는 후속이다. 기존30일 보존/삭제 cascade/필수지표는 유지한다.

시나리오를 먼저 기록하고 실제 router의 미설정 경로/인증·폐쇄입력/동의거부 Red를 실행한다. 이후 HTTP fake port 경계와 실PG 동일receipt·목적거부/철회/타세션·충돌·권리취소·원래기한/backend종료·실HTTPS 미동의차단을 검증한다. 문서/Mermaid·생성TS·정적검사·전체Rust/PG/web/CI95/80과 정확head·코드리뷰 뒤 병합한다. 미실행 검사는 결과로 보고하지 않는다.
