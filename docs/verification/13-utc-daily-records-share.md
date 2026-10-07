# #13 UTC 솔로 데일리·개인 기록·공유 검증

PR40 초기 head6812542의 웹 CI는64개 실제 키보드 입력을 통과하는 데일리 완주 검사의30초 전체 예산에서 PC/모바일 timeout을 보였으며 모바일은 retry도 실패했다. 병행 renderer 측정도60초 timeout 뒤 retry로 통과했다. 기능 검사40개가 먼저 끝난 뒤 독립 measurement project에서 PC/모바일 각각 한 context로120표본을 순차 수집하도록 바꿨다. 완주+DB+PNG+reload 검사만60초 전체 예산을 사용하고 개별 상태 assertion5초·표본120개·FPS16.7ms 목표는 유지한다. 소프트웨어 CI의 GPU 간섭을 줄이는 검사 구조 변경이며 제품 성능 목표 달성으로 세지 않는다. 수정 뒤 로컬 전체42개 통과, 최신 CI는 별도 확인한다.

FR09, TS17/18/36 · [ADR0017](../adr/0017-deterministic-solo-daily-and-local-records.md). 제품 코드 `8f7e5b6eddbe35492d7b32962778d2d6460f05e2`, 선행 PR39/#12의 병합·종료를 확인한 뒤 구현했다.

엄격한 Gregorian UTC 날짜·seed version·solo mode·rules hash·solver/RNG version을 canonical BLAKE3 공개 seed에 묶는다. 같은 날짜의 native/WASM 판과 입력 결과가 같고 날짜/윤년·버전/숫자 truncation 오류를 거절한다. shared RuleEngine의 솔로 모드는 open/flag만 허용하며 공격·상대 입력 거절도 idempotent다. zero만으로 전부 연 솔로 판의 clear와 대전의 동시 clear draw를 구분한다. timeout은 퍼즐 완료가 아니며 clear의 개인 elapsed만 고정한다.

Rust DailyView는 own-only이고 TS가 규칙을 재구현하지 않는다. 공개 daily seed는 온라인 비밀 seed와 구분하며 카드에 넣지 않는다. strict replay는 malformed/enum extra field·metadata/date/mode/seed binding·시간 역행·중복·8192입력/2MiB 한도를 검사한다. 공개 no-lie solver가 만든 완료 fixture를 native roundtrip하고 실제 Chromium WASM 전체 snapshot/ack/replay와 비교했다. 이 replay만으로 사람의 플레이나 실제 시간을 증명하지 않으며 서버 공식 검증/순위는 #19다.

`DailyPage`는 시작 UTC 날짜를 고정하고 자정/locale 변경에도 세션을 유지한다. 새 퍼즐만 wall clock 날짜를 다시 선택한다. Worker와 기존 FIFO/monotonic controller·terminal 정리를 사용하며 Board는 Rust 허용 액션으로 버튼·키보드·메뉴를 제한한다. IndexedDB 첫 local clear는 한 readwrite transaction에서 저장하고 재시도는 연습으로 표시한다.30개 상한, reload/clear·동시 두 repository·strict unverified 저장 형식·손상 데이터·denied/stalled5초 메모리 fallback을 검사했다. 이름·계정·비밀번호·이메일을 저장하지 않는다.

공유는 날짜/버전/전체 안전 진행/개인 시간/실수/미검증 표기만 allowlist한다. 장식32tile mosaic는 실제 위치/숫자/지뢰와 무관하다. clipboard 복사·Web Share adapter·공통 토큰 Canvas800×600 PNG를 제공한다. PC 실제 clipboard와 PC/모바일 실제 PNG signature/크기·파일 이름을 확인했다. Web Share의 실제 외부 전달은 실행하지 않았다. 공유/저장소 실패에도 게임과 재시작을 허용하며 계정/광고/API 요청0을 확인했다.

- `scripts/check.ps1` 종료0: fmt/clippy·전체 Rust·WASM·DTO/WASM/fixture/token freshness·format/lint/typecheck·Vitest49개·build·Chromium PC/모바일42개·문서89개/Mermaid34개. PowerShell5의 stderr 전체 리디렉션은 `Checking chrono`를 NativeCommandError로 처리해 처음 실패했으며 직접 자식 PowerShell 프로세스로 재실행했다. 코드 오류로 세지 않는다.
- 코어 llvm-cov line2314/2366=97.80%. 웹 V8 line817/935=87.37%, statement878/1017=86.33%, function264/309=85.43%, branch522/663=78.73%. main 포함, 테스트/setup 외 source 제외 확대 없음. [측정 JSON](daily-summary.json).
- 실제 날짜 자정/locale 유지·WASM 완주/최초 기록·재조회·PNG·clipboard·denied 저장소/복사·실제 페이지4분 timeout≠completion·새 퍼즐을 검사했다. [PC](screenshots/13-daily-chromium.png)·[모바일](screenshots/13-daily-mobile.png)은 실제 페이지이며 Pixel7은 viewport 에뮬레이션이다.

manifest 정적 초기 JS gzip31860byte+선택 locale 최대3659byte=35519byte다. 데일리 정적 추가6782byte, Pixi108639byte, core792byte이며 공유 dependency가 있어 단순 합산을 실제 요청량으로 쓰지 않는다. 전체 JS assets229802byte는8locale/backend를 포함한 보수적 상한이고 단일 게임 경로가 아니다. WASM486773byte/gzip179615byte·glue22284byte/gzip3035byte는 별도다. 실제 mini PC/FPS/동적 game 요청 예산은 #26, 미캐시 오프라인/대기 제출은 #14, 공식 완료/시간/순위는 #19다.

코드/diff 리뷰에서 public/secret 경계·규칙 단일 원천·로그/저장소 상한·첫 기록 원자성·늦은 Worker/replay 정리·locale 세션 유지·개인 미검증 표기·8언어145키·무계정/무광고를 확인했다. 로컬 PostgreSQL1개 ignored는 통과가 아니며 PR의 필수 실제 database CI와 구분한다. PR 최신 head의6checks 성공 후 위임에 따라 squash merge하고 이슈 종료를 확인한다. E1/E2 사람 관찰은 미실행이며 M2/#27 수동 게이트를 유지한다.
