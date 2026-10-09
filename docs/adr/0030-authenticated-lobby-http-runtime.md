# ADR0030: 인증 로비 HTTP와 실행 구성

2026-10-09 · #64/M3 · 상위 #17 · FR01/02/06/07/14/16, NFR02 · TS10/12/20/21/31~36. 선행 #62/PR63 developba91e66 병합·종료.

## 전송과 공개 상태

로비는 POST /api/v1/lobby 하나의 versioned JSON intent(status/queue_join/room_create/room_join/ready/cancel)를 사용한다. 게임 입력은 기존 인증 WS를 유지한다. status도 준비 완료를 registry로 commit할 수 있으므로 모든 로비 호출에 정확 Origin·session-bound CSRF를 요구한다. 브라우저는 기존 bootstrap의 메모리 CSRF를 사용한다. OAuth query/초대는 별도 거래이며 로비 URI의 query는 거절한다. DTO는 Rust 원천→생성 TS이며 TS에서 FIFO/규칙을 만들지 않는다.

공개 응답은 version/server_time_ms와 본인 idle/queued/room/preparing/matched/failed 상태다. queue/room UUID·canonical8문자 코드·공개 occupied/ready·own seat·server deadline·선택 난이도와 human/bot 종류만 전달한다. preparing 취소 identity는 entity UUID와 단조 generation의 문자열이다. 상대 account·nickname/세션·내부 seed/Board·core global revision은 전달하지 않는다. generation 문자열로 JSON/TS 정밀도를 보존한다. 취소는 source queue/room/preparing identity를 다시 검증하며 과거 요청이 새 membership을 지우지 않는다.

크기1024byte·닫힌 enum/version/canonical UUID·중복/unknown field를 거절한다. room code의 실제 alphabet/수명/좌석은 기존 pure domain이 결정한다. 오류는 안정된 공개 code와 HTTP 상태이며 내부 오류/secret을 포함하지 않는다. 모든 응답은 no-store/no-referrer/nosniff다.

## 인증·자원·열거

Origin/CSRF/쿠키/JSON을 확인한 뒤 global 동시 session-read permit과 bounded session-hash rate를 적용한다. authority generation은2초 DB read 전에 캡처하고 완료 후 nickname/expiry/현재 generation의 bind_shared를 검사한다. LobbyService의 membership과 admitted actor가 이후 권한 수명을 소유한다. 외부 account/seat 주장은 받지 않는다.

세션20회/초와 계정20회/초를 제한하며 room_join은 유효 계정당5회/분이다. 세션을 회전해도 같은 계정의 열거 제한을 공유한다. 두 rate map은 각4096entry/60초, 시각 역행·잠금 실패·포화는 fail-closed다. IP·browser fingerprint를 저장하지 않는다. permit은 read/응답까지 실제 request가 소유하며 timeout/cancel 때 반환한다. 수치는 출시 성능 실측을 대신하지 않고 #26에서 평가한다.

## 실행 구성과 비밀 수명

main은 같은 SystemMatchClock/MatchRegistry, OS entropy 인증 보드 풀, 실제 core bot executor, LobbyService와 HTTP/WS를 연결한다. lobby와 socket authority는 별도 registry이며 CombinedSessionInvalidator를 persistent store에 주입한다. BrowserSecurity는 Arc로 auth와 lobby가 공유하여 CSRF origin/key가 일치하고 secret 복제를 추가하지 않는다. zeroizing key는 마지막 소유권 반환 때 제거한다.

로비 capacity32/preparation workers2/authority64/HTTP requests16, 보드 pool4/workers1, bot workers2가 개발 기본값이다. 환경변수의 빈 값/부호/공백/과도한 값과 workers>capacity는 시작을 거절한다. auth가 비활성이면 로비도 unavailable이고 계정 없는 local practice는 유지한다. migration/외부 등록/키는 기존 별도 절차이며 자동 생성하지 않는다.

## 검증과 남은 출구

닫힌 Rust DTO와 wire 비공개 필드, 인증 전 storage 미호출, session read 중 철회·timeout·global bound, 세션 회전 뒤 계정 room rate, 실제 HTTP 큐/친구방·identity 취소→registry/WS, 실DB nickname/session/철회 및 runtime 구성의 Red→Green을 검사한다. 최신 native/WASM·생성 선언/웹·DB·coverage/문서를 확인한다.

OAuth 초대 거래와8언어 로비/온라인 화면·클라이언트 재접속은 후속 통합 출구다. 상위17은 OPEN이며 실제 키42·부하26·사람27을 대체하지 않는다.
