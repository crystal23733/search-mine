---
name: liar-design-review
description: Review Liar Sweeper PRD, game fairness, design changes, and requirement-to-test traceability before product implementation or a rule change.
---

# 설계 검토

저장소 루트 기준 `docs/product/16-PRD.md`, `docs/design/00-review-checklist.md`, 변경된 상세 설계와 ADR을 읽는다. 승인 상태는 `docs/workflow/design-approval.json`에서 확인한다.

관측 정보만으로 안전한 추론과 lie 식별이 가능한지 확인한다. 숨은 정답 하나에서만 성립하는 시뮬레이션이나 나중의 모순은 충분한 증거가 아니다. 두 overlay·탐색 예산·실패시 미적용·봇 정보 제한을 같이 평가한다. 온라인 seed/정답과 공개 데일리를 구분한다.

PRD ID→설계→시나리오→테스트 포트→백로그 연결을 갱신한다. SRP/DIP, 8언어, 광고/동의, 오프라인 신뢰, 서버 내부 성능과 글로벌 RTT, 복구 조건의 충돌을 찾는다. Mermaid와 링크를 실제 검사한다.

결과는 해결한 문제·남은 가정·승인할 제안·실행한 검사를 구분해 기록한다. 사용자 승인 증거 없이 pending을 approved로 바꾸거나 제품 구현을 시작하지 않는다. 범위 변경은 구체 설계안을 작성한 뒤 사용자 검토를 받는다.
