# #9 public WASM·native 재현 검증

NFR01/02 · TS02/22/30. [ADR0013](../adr/0013-public-wasm-and-native-contract.md), 코드 `93866018e77ba3c780225c76d3f0424fd4012767`. 선행 #8은 PR35 병합·이슈 종료를 확인했다.

Rust protocol DTO→ts-rs→TS, production LocalSession→wasm-bindgen→JS/선언, PracticeCore DI→module Worker로 연결했다. 공개 DTO에는 본인 화면 셀·flag·gauge·stun·숫자 변경 이력·실수/지목 집계, 상대 progress/stun, rules/hash·본인 revision·결과만 있다. Board/seed/overlay/truth/lie marker/후보·상대 셀은 받지 않으며 private 타입에 Serialize/Debug를 추가하지 않았다. 연습 생성자의 공개 seed는 온라인 모드로 전달하지 않는다.

실행한 Red: 유효 flag 입력을 adapter stub이 Malformed로 거절; Worker의 순서가 바뀐 성공 응답을 잘못 거절; 실제 WASM에서 `advance(-1)`이 정수 변환 뒤 큰 양수가 되어 매치를 종료; 실수 집계가0으로 남음. 각각 Green으로 수정했다. JS clock은 finite integer/range 검사 뒤 변환하며 잘못된 시간·프레임은 상태를 바꾸지 않는다. 최신 생성 선언의 Symbol.dispose 타입을 검사하기 위해 ESNext.Disposable lib를 추가했고 skipLibCheck는 사용하지 않았다.

- Rust73개 통과: core64, protocol4, server2, wasm3. 실제 PostgreSQL1개는 로컬에서 ignored이며 필수 database CI에서 별도로 실행한다.
- Vitest4개 통과: 브랜드 진입과 Worker out-of-order/unknown ID, pending64 한계, 종료·timeout·공개 error 전달.
- 실제 Chromium desktop/mobile12개 통과: 기존 진입2, native/WASM 동일 seed3×2, production Worker2, JS 숫자 경계2. mobile은 Pixel7 viewport 에뮬레이션이며 실기기 측정이 아니다.
- native fixture는 seed0/42/u64::MAX, 세 난이도의 초기 공개 snapshot과 각13개의 flag/dedup/flagged open/unflag/open/잘못된 frame·version/attack/accuse/clock/timeout 결과를 포함한다. Rust 생성 결과 전체와 실제 WASM JSON을 비교했으며 fixture freshness도 검사한다.
- 직렬화 리뷰/테스트는 숨은 공격 직전·직후 방어자 public JSON의 동일성, 화면의 실제 표시 숫자와 closed/mine의 number null, 상대 progress/stun만 전달, abort/cancel의 미완료 구분을 확인했다. command 중복은 본인 실수/지목 집계를 늘리지 않는다.

fmt/clippy·WASM target·Rust DTO/wasm-bindgen 선언 freshness·native fixture freshness·lint/typecheck·unit/build·E2E·문서/Mermaid 검사를 통과했다. web 필수 CI가 최신 CLI 설치부터 실제 WASM 빌드·재생까지 수행한다. [CLI](https://wasm-bindgen.github.io/wasm-bindgen/reference/cli.html)와 [web 배포 계약](https://wasm-bindgen.github.io/wasm-bindgen/reference/deployment.html)을 공식 문서에서 확인했다.

core 소스 전체(테스트/예제 제외) line2133/2180=97.8440%, function181/185=97.8378%, region3267/3389=96.4001%. [측정 요약](wasm-coverage-summary.json). branch/전체 앱 coverage는 이 측정에 포함하지 않았다. release WASM409780byte, gzip154908byte로 WASM1MiB 예산 안이다. 웹 JS entry gzip은 약5.05kB이며 게임 UI는 아직 연결 전이다. Worker lazy chunk와 게임 bundle은 실제 화면에 연결하는 #12 및 #26에서 측정한다. CPU/글로벌 지연/실기기 FPS 결과로 확대 해석하지 않는다.

에이전트 리뷰는 공개 projection allowlist, per-viewer revision, strict JSON byte/type/unknown-field/version/ID 검사, canonical 공개 이력·봇 RNG 분리, Worker 종료와 요청 연결, 최신 lockfile 및 생성 artifact 경계를 확인했다. 화면/튜토리얼/daily/온라인 연결은 후속 이슈에서 진행한다. 필수6개 CI가 실제 PR head에서 통과한 뒤 위임에 따라 병합한다.
