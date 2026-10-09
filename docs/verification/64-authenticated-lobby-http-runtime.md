# #64 검증: 인증 로비 HTTP와 실행 구성

2026-10-09 · #17/M3 · [ADR0030](../adr/0030-authenticated-lobby-http-runtime.md) · FR01/02/06/07/14/16, NFR02 · TS10/12/20/21/31~36. 선행 #62/PR63 developba91e66 병합·종료.

## 실제 Red→Green

Rust wire 계약3개는 actualRed1passed/2failed→Green3이다. versioned intent·canonical UUID·중복/unknown field·1024byte·u64 generation 문자열을 검사한다. enum의 serde 지원 경고를 임의 suppress하지 않고 기존 online 계약처럼 typed parse와 JSON roundtrip의 필드 일치를 사용한다. 생성 TS는 원천에서 갱신했으며 게임 규칙은 TS에 복제하지 않았다.

로비 구성2개는 actualRed1/1→Green2다. 7개 개발 기본값/override·빈 값/부호/공백/상한·worker>capacity를 확인했다. HTTP9개는 actualRed0/9→Green9(2.02s)다. 전 호출 Origin/session-bound CSRF·권한 없는 nickname/expiry/session·query/body/JSON 크기·no-store/no-referrer/nosniff, read 중 철회/generation, global read permit/실제 cancel/2초 timeout, 세션·계정 rate와 계정당 room_join5회/분을 검사했다.

실제 HTTP의 두 큐가 같은 registry ID/다른 own seat를 받고 TCP WS가 같은 공개 board/countdown을 관측하며 Rust 입력의 Applied ack를 받았다. 합성 account 철회가 실제 socket을 닫는다. 이미 전송한 delta는 수신 buffer에 남을 수 있어 closure까지 관측했다. 친구방 ready→unready→새 generation 뒤 과거 preparing cancel은 Stale이며 현재 상태를 유지한다. membership 없는 outsider의 cancel은 기존 idempotent Idle 계약을 유지하고 실제 방은 바뀌지 않는다.

추가 리뷰 probe는 서로 다른21개 유효 session의 동일 account burst와 요청 Hard/실제 Normal의 경계다. 변경된 난이도 join은 기존 정책이 Busy로 거절하고 status는 원래 Normal을 보존한다.10초 backfill 뒤 bot 표시·실제 core bot의 첫 공개 진행을 검사했다. HTTP 최종11Green(2.03s)이며 요청값을 실제 값으로 간주하지 않는다.

## main과 실제 저장

실행 파일2개는 운영체제 필수 환경을 보존한 isolated loopback에서 actualRed0/2(로비404·잘못된 구성의 미종료)→Green2(1.87s)였다. 처음 env_clear가 Windows 필수 환경까지 지워 서버가 종료한 결과는 제품 Red로 계산하지 않는다. fixture child는 Drop에서 종료/회수하며 사용자 서버를 종료하지 않는다.

main은 같은 clock/registry에 실제 pool/core bot/LobbyService·HTTP/WS를 조립한다. persistent store에 두 namespace의 합성 invalidator를 주입하고 BrowserSecurity는 auth/lobby가 Arc로 공유한다. 키 없는 실행은 lobby503/무계정 연습 유지, 활성 실행은 인증 bootstrap CSRF의 일치·세션 없음401, 잘못된 구성은 serving 전에 종료한다.

25번째 PostgreSQL 시험은 실제 저장된 nickname/session으로 HTTP 배정→persistent logout→actor 초기 Cancelled→원자 결과/두 참여자 기록→상대 Idle을 검사한다. 아직 실행하지 않은 로컬 실DB를 성공으로 계산하지 않는다. scripts/check.ps1 전체 exit0(native/WASM/생성 선언/웹90unit/browser50/문서116/Mermaid34)와 추가 HTTP11/all-target clippy exit0를 확인했다. 이후 문서 추가와 최신 CI의 실제 DB·coverage·browser 결과는 PR와 로컬 review에 기록한다.

## 리뷰와 남은 범위

pm-skills의 정확성·보안 scope64/baseba91e66을 순차 검토했다. 닫힌 cookie/CSRF/JSON→bounded read→캡처 generation/완료 expiry→shared lease→동일 계정 rate→domain의 실제 상태→공개 DTO/WS, main의 shared security/두 namespace→persistent logout→actor 결과를 함께 조사했다. 요청 난이도 변경과 새 preparing/old cancel을 강제해 실제 상태의 권위/identity를 확인했다. namespace/key·동시 query·permit Drop·시각 역행·rate 포화/정확 만료도 확인했다. 검토 범위에서 추가로 뒷받침되는 결함은 남지 않았다.

OAuth 초대 거래·8언어 로비/온라인 화면·클라이언트 재접속은 후속 통합 출구다. 이 서버 API 구현만으로 웹 온라인 대전 완료를 주장하지 않는다. 상위17은 OPEN, 실제 키42·미니PC26·사람27은 별도 출구다.
