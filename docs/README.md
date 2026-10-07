# 문서 안내

1. [제품 비전](product/01-vision.md)과 [PRD](product/16-PRD.md)로 범위·성공 기준을 읽는다.
2. [설계 리뷰·요구 추적표](design/00-review-checklist.md)에서 승인할 제안을 확인한다.
3. [시스템 구조](design/01-architecture.md), [규칙](design/04-game-rules-spec.md), [간파 알고리즘](design/05-algorithms.md), [프로토콜](design/06-protocol.md)을 검토한다.
4. [테스트 시나리오](product/19-test-scenarios.md)와 [백로그](workflow/backlog.md)에서 구현·검증 단위를 확인한다.
5. [Git workflow](workflow/git-workflow.md)와 [스킬 적용 내역](workflow/skill-usage.md)을 따른다.

## 문서 묶음

- `planning/`: 제공된 브레인스토밍·핸드오프 원문, 내용 그대로 보존.
- `product/01~21`: 비전·경쟁·세그먼트·여정·가치·경제·리스크·요구·테스트·지표·로드맵.
- `design/01~18`: 구조·배포·도메인·규칙·알고리즘·프로토콜·DB·client·UX·언어·광고·보안·성능·테스트·repo·운영·OAuth/개인정보·제공된 디자인 기준.
- `adr/0001~0014`: 공유 코어·DB·홈서버·Preact·Pixi·권위·간파 증명·workflow·최소 OAuth·공개 후보 전략·규칙 snapshot·공개 관측 봇·public WASM·Atomic shell/locale의 결정.
- `workflow/`: develop·이슈/마일스톤·승인 상태·작업 백로그·스킬 출처.

현재 M0/M1과 #4~9 구현은 develop에 병합됐고 [#10 Atomic shell 검증](verification/10-atomic-shell.md)을 마쳤다. 승인/실행 결과/가정을 구분한다. 참조 기술 문서는 각 실제 구현/공개 시 다시 확인한다.
