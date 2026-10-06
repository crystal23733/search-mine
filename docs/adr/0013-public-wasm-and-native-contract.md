# ADR0013: 공개 DTO·로컬 WASM·native 재생 계약

- 날짜: 2026-10-07
- 상태: 채택 — #9, NFR01/02, TS02/22/30. 선행 #8은 PR35 병합·종료 확인.

## 결정

protocol crate의 명시적인 public DTO를 native 서버와 WASM adapter가 공용으로 사용한다. 본인 셀 state/화면 number/flag·gauge·stun·공개 숫자 이력·본인 실수/지목 집계·상대 progress/stun·규칙 snapshot·본인 stream revision·결과만 직렬화한다. seed·Board/board hash·상대 cells·overlay/source·논리 truth·lie flag·후보 목록은 없다. core의 private 타입에 Serialize를 추가하지 않는다. public Rust DTO와 wasm-bindgen 선언에서 TS를 생성하고 freshness를 검사한다.

WASM LocalSession은 공개 연습 seed와 난이도로 동일 Rust RuleEngine을 만든다. 온라인 모드에는 seed 생성자나 LocalSession을 연결하지 않는다. 본인 command→공개 step 결과, clock tick→공개 상태, dispose만 노출하며 JS getter로 숨은 Board를 열지 않는다. 로컬 completion은 공식 검증 완료로 승격하지 않는다. Worker에서 계산하고 main UI에는 public DTO만 전달한다. 요청 ID별 응답 연결·pending 한계·dispose의 대기 요청 종료를 검증한다.

local input은 v/command_id/client_seq/action의 Rust 계약이다. action은 open/flag/accuse의 cell 또는 target 없는 attack이다. JSON byte8192 초과·negative/out-of-range 타입·unknown field/version/action·ID/sequence0을 경계에서 거절한다. WS envelope는 #16의 기존 계약으로 감싸되 같은 public action을 사용하고 client seat/seed를 받지 않는다.

native fixture runner가 public seed·clock/input 목록과 예상 public JSON을 만들고, 같은 production WASM LocalSession을 실제 Chromium에서 초기화해 replay 결과를 비교한다. 보드 생성·숫자/flag·기절·dedup·규칙 hash·봇 진행·잘못된 입력을 검사한다. WASM target compile만으로 재현 일치를 주장하지 않는다.

wasm-bindgen CLI는 버전 인자 없이 최신을 설치하고 crate와 호환 여부를 실제 build로 확인한다. generated JS/WASM은 build artifact이고 공개 TS 선언/DTO는 저장소에 보존한다. `--target web`의 ES module과 명시적 초기화를 사용한다. [공식 CLI](https://wasm-bindgen.github.io/wasm-bindgen/reference/cli.html), [배포 계약](https://wasm-bindgen.github.io/wasm-bindgen/reference/deployment.html).

## 후속 경계

Atomic UI·보드 입력은 #10/#11, 로컬 봇/튜토리얼은 #12, daily/offline은 #13/#14, 인증/온라인 authority는 #15/#16에서 연결한다. production WASM/browser/native fixture·bundle 결과와 실호스트 성능을 구분한다.

JS clock is validated as a finite integer in 0..u32::MAX before conversion; negative/fractional values cannot mutate the game clock. Latest generated Symbol.dispose declarations use ESNext.Disposable types; lifecycle uses explicit free/Worker terminate.
