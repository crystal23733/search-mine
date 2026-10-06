---
paths:
  - "crates/**"
  - "apps/**"
  - "packages/**"
  - "tests/**"
---

# 구현 경계

SRP/SOLID·DIP, 제품 코드 TDD, Rust 단일 규칙, public DTO 분리, Atomic Design·공통 tokens, 8언어·동의/광고 무차단을 따른다. 관련 명세는 `docs/design/03-domain-model.md`, `08-client-architecture.md`, `14-testing-strategy.md`에 있다. 성능은 측정해서 보고한다.

OAuth는 provider port/adapter로 확장하고 Google·Apple·카카오·네이버가 필수다. 비밀번호·이메일/실명/전화/사진 권한이나 저장 필드를 추가하지 않는다. 17 인증/개인정보와 18 디자인 기준을 따른다. 계정 연결은 재인증으로 명시적으로 수행한다.
