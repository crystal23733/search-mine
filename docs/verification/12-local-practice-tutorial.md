# #12 로컬 봇 대전·튜토리얼 검증

FR06/08, TS11/13/36 · [ADR0016](../adr/0016-local-match-controller-and-training-fixture.md). 코드 `1f7bc94500c9ed07010444b804de52e21f0f6393`, 선행 PR38/#11의 병합·종료를 확인한 뒤 구현했다.

`/locale/practice`는 실제 WASM RuleEngine과 공개 관측 BotPolicy의 easy/normal/hard를 사용한다. 로컬 CSPRNG seed·Worker·단일 FIFO controller, 양쪽 동일 zero 오프닝·countdown·공개 진행/시간/gauge/stun/stat/result를 연결했다. UI는 성공·숫자·승패를 추측하지 않고 public view/ack를 반영한다. 입력 ID/seq 재전송, 64입력 상한·tick 합치기, 처리 시각·terminal·늦은 init·unmount·실패 시 타이머/Worker 정리를 검증했다. 난이도/새 대전은 세션을 교체하고 locale/접근성은 세션을 유지한다.

최초 방문과 명시적 tutorial route는 `config/tutorial.toml`의 공개3×3/2mine/gauge1 학습 판을 사용한다. 실제 Rust safe gain·공격 인증·상대 공격·지목·2초 반사와 Rust 원천 TutorialView/Step을 연결했다. C3 열기→공격→C1 열기→지목 뒤 숫자2→1 복원과 상대 기절을 확인했다. 화면에서 일반16×16/gauge20과 구분한다. 학습 완료는 일반 match 완료/순위/광고 빈도에 넣지 않는다.

new/skipped/complete만 필수 로컬 저장소에 남긴다. 손상/읽기·쓰기 거부는 메모리 fallback과 유실 안내이며 skip/연습은 계속된다. locale·초대 query/hash·난이도를 유지하며 return은 내부 allowlist만 허용한다. OAuth/계정/광고/API 요청 없이 실행했다. Rust/Web/WASM의 초기 행동 Red 뒤 Green을 확인했고 순서 오류가 학습을 종료시키던 경로를 추가 Red로 수정했다. 잘못된 동작은 현재 공개 view와 rejected ack를 반환하고, 제한된 거절 메모로 동일 ID가 단계 변경 뒤 적용되지 않게 한다.

- 최종 `scripts/check.ps1` 종료0: fmt/clippy·Rust·WASM·DTO/fixture/token freshness·format/lint/typecheck·Vitest41개·build·실제 Chromium PC/모바일36개·문서87개/Mermaid34개 통과. 로컬 PostgreSQL1개는 ignored이며 PR 필수 database job의 실제 검증과 구분한다.
- 코어 llvm-cov line2232/2281=97.85%. 웹 V8 line623/677=92.02%, statement673/737=91.31%, function196/218=89.90%, branch389/472=82.41%. 미import main을 포함하고 테스트/setup만 제외했다. [측정 원자료·번들](local-practice-summary.json).
- 실제 Chromium에서 최초 진입/잘못된 학습 동작/공격/지목/반사·locale 유지·완료/replay·skip/초대/외부 return·저장소 거부·3난이도 실제 봇 safe 증가·재시작/해제를 검증했다. 봇 초기 진행0이라는 잘못된 assertion은 양쪽 zero 오프닝의 실제 초기값 이후 증가를 검사하도록 수정했다.
- [PC 대전](screenshots/12-match-chromium.png)·[모바일 대전](screenshots/12-match-mobile.png)·[PC 학습](screenshots/12-tutorial-chromium.png)·[모바일 학습](screenshots/12-tutorial-mobile.png)은 실제 페이지다. 모바일은 안내→보드→요약, PC는 보드 좌측·안내/요약 우측이다. Pixel7 viewport는 에뮬레이션이며 실기기 성능 자료가 아니다.

manifest 정적 import closure의 초기 JS gzip28471byte, 선택 언어 추가 최대2988byte, 초기 상한31459byte다. 대전/학습 정적 추가6145/6131byte, Pixi 정적 추가108859byte, core adapter728byte이며 공유 dependency가 있으므로 이 값들을 단순 합산해 실제 요청량으로 쓰지 않는다. 전 backend·8locale·game assets 합219587byte는 보수적 전체 상한이다. WASM436234byte/gzip163295byte, glue16529byte/gzip2872byte는 별도다. 실제 단일 게임의 동적 요청 경로와 CPU/GPU/미니 PC 성능은 #26에서 측정하며 전체 game200KiB·60fps를 달성했다고 주장하지 않는다.

실제 참여자의 30초 이해·재미 E1/E2는 미실행이다. [관찰 절차](12-tutorial-observation-guide.md)를 준비하고 #27에 추적했으며 M2의 해당 수동 출구 게이트는 유지한다. 코드/diff 리뷰에서는 비공개 DTO 제외·규칙 단일 원천·중복/거절 상한·CSPRNG 실패 전 Worker 생성 방지·async 정리·first visit/return·8언어117키·무광고/무계정을 확인했다. 최신 PR head의 필수6checks를 확인하고 위임에 따라 병합한다.
