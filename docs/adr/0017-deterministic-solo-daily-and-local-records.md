# ADR0017: UTC 솔로 데일리·개인 미검증 기록·공유 경계

- 날짜: 2026-10-07
- 상태: 채택 — #13, FR09, TS17/18/36. PR39/#12 병합·종료 확인.

## 결정

데일리는 같은 UTC 날짜에 같은 노게스 솔로 퍼즐을 제공한다. FR09의 퍼즐은 상대와 경주하는 FR06 대전과 구분한다. 기본16×16/40mine/zero 오프닝/4분/기절 규칙은 공유 RuleEngine에서 실행하고 봇이나 상대의 공격을 만들지 않는다. 따라서 숨은 거짓 숫자가 없고 open/flag만 허용하며 화면에 솔로 퍼즐임을 안내한다. 공격/지목 학습과 대전은 기존 경로를 유지한다. 수치는 TS에 복제하지 않고 실제 RulesSnapshot과 Rust가 제공한 허용 액션을 사용한다. 시간 종료는 퍼즐 완료로 세지 않으며 모든 안전 칸을 연 clear만 완료다.

Rust daily 원천은 엄격한 YYYY-MM-DD Gregorian 날짜, seed version1, mode `solo-v1`, RulesSnapshot hash·solver/RNG version을 canonical 문자열에 넣고 domain-separated BLAKE3의 첫8byte little-endian으로 공개 u64 seed를 만든다. 동일 metadata는 native/WASM/locale에 관계없이 같은 Generator 판과 입력 결과를 만든다. online seed/정답과 다르게 이 seed는 공개 데일리 메타데이터이고 공유 카드에는 넣지 않는다. 날짜/버전 오류를 거절하고 시작 시 snapshot/date를 고정해 UTC 자정/언어 변경이 진행 중 판을 바꾸지 않는다. 다음 새 퍼즐부터 주입된 UTC wall clock을 사용한다.

RuleEngine의 한 seat만 입력하고 다른 seat는 자동 오프닝 이후 진행하지 않는다. 공개 DailyView는 metadata·own board/phase/time/stat·clear 여부·개인 경과시간과 허용 액션만 포함하고 상대 경쟁 요약을 표시하지 않는다. elapsed는 규칙 snapshot의 duration/countdown과 실제 engine 상태에서 계산하며 클라이언트 시간임을 안내한다. replay는 strict LocalInput+time 로그와 final tick·metadata를 보관하고 입력 수는 snapshot command 한도로 제한한다. malformed·역행 시간·duplicate/종료 경계를 실제 Rust에서 검증한다. #19 서버 verifier는 이 Rust daily contract를 사용하고 공식 완료/시간은 서버가 판단한다.

개인 저장소 port는 IndexedDB adapter와 memory fallback으로 구현한다. daily ID별 첫 local clear는 한 readwrite transaction에서 저장하며 이후 재시도는 연습으로 표시한다. 최대30개 daily ID를 유지하고 내보내기/삭제·대기 제출은 #14/#20과 연결한다. 저장 항목은 v1·공개 metadata·로컬 attempt UUID·개인 elapsed/stat·native replay뿐이며 계정/이메일/닉네임/비밀번호가 없다. 저장 거절·용량/blocked/malformed 값은 unverified 기록을 유지하거나 유실 안내하고 플레이/공유를 막지 않는다. 로컬 값으로 verified나 공식 순위를 만들지 않는다. 무계정 기록의 로그인 후 제출은 #19의 명시적 동작이다.

공유 port는 날짜·공개 버전·완료/안전 진행 비율·개인 참고 시간·실수·unverified 표기만 허용한다. mosaic는 전체 진행 비율로 채운 장식32칸이며 원래 보드 위치/숫자/지뢰/입력 로그와 무관하다. text 복사, 사용자가 누른 Web Share(지원 시), PNG 다운로드를 제공하고 실패는 다른 경로를 안내한다. 카드 exporter는 같은 허용 DTO와 공통 토큰을 사용한다. 참가자 이름·계정·seed·정답·좌표를 카드에 넣지 않으며 카드 생성/복사/다운로드가 서버나 광고 요청을 만들지 않는다.

TDD는 날짜/윤년/버전·동일판·솔로 종료·로그 상한/중복/시간/완료, native/WASM fixture, UTC 경계·locale 세션 고정, 첫 기록 atomic 저장·손상/거부, 공유 allowlist·실제 PNG/clipboard·8언어·모바일을 검증한다. 캐시 없는 오프라인 최초 접속과 공식 제출/순위는 #14/#19에서 구현하며 이번 로컬 기록을 공식으로 표시하지 않는다.

## 기술 근거

zero 오프닝만으로 모든 안전 칸이 열리는 판도 countdown 종료에서 솔로 완료로 판정한다. 공유 엔진의 대전에서는 기존 동시 clear 무승부를 유지하고 솔로에서는 seat1의 clear로 처리한다.

[Chrono 공식 기능 문서](https://docs.rs/chrono/latest/chrono/)의 std/alloc와 [NaiveDate 파싱](https://docs.rs/chrono/latest/chrono/struct.NaiveDate.html)을 확인했다. 버전 인자 없이 cargo add로 최신 안정 릴리스를 설치하고 clock/OS timezone/JS Date binding을 켜지 않는다. wall clock과 monotonic session clock은 외부 port가 제공한다.
