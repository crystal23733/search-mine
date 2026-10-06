---
name: liar-tdd
description: Implement an approved Liar Sweeper product issue using documented acceptance scenarios, Red Green Refactor, and the Rust core and client service boundaries.
---

# 승인된 이슈의 TDD

루트 `AGENTS.md`, `docs/workflow/design-approval.json`, 해당 GitHub 이슈와 `docs/product/19-test-scenarios.md`를 확인한다. pending이면 문서 정리까지만 수행한다. 이슈의 선행 PR이 병합되어야 새 구현을 시작한다.

시나리오의 관측 결과를 테스트로 옮겨 실패를 실행 확인한다. fake Clock/RNG/Repository/Transport 또는 TS service port로 경계를 제어하되 실제 DB·WS 계약은 통합 테스트에서 확인한다. 동작을 최소 구현하고 통과 후 책임·중복을 정리한다.

게임 규칙을 TS에 복제하지 않는다. public DTO와 내부 Board를 분리하고 native/WASM 결정론 fixture를 사용한다. 변경된 규칙/프로토콜은 설계·ADR부터 갱신한다. generated TS는 원천에서 다시 만든다.

변경에 필요한 단위/속성/통합/E2E/성능 검사를 실행하고 결과와 미실행 환경을 PR에 적는다. 커버리지 목표·예산을 실제 측정과 구분한다. 완료 후 `liar-issue-pr`의 PR 제출 흐름을 따른다.
