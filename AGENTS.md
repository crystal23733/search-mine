# Liar Sweeper 작업 지침

## 현재 단계와 우선순위

이 저장소는 사용자 설계 변경·개발 시작 지시를 반영했고 설계 PR 병합을 기다린다. 최신 사용자 지시가 우선이고, 승인된 PRD/설계/ADR→이 문서와 workflow→docs/planning 원문 순으로 해석한다. 원문의 Next/Nest·스팀·포털·Cloudflare DO 아이디어는 채택하지 않았다. 오래된 HANDOFF의 브랜치/커밋 관례는 [새 workflow](docs/workflow/git-workflow.md)가 대체한다.

`docs/workflow/design-approval.json`과 승인 근거를 읽는다. OAuth/화면 변경과 구현 시작은 승인되었지만 **승인 기록 PR이 develop에 병합되고 선행 이슈가 닫힌 뒤 제품 작업을 시작한다.** 2026-10-07 사용자가 이후 모든 작업의 검토·병합·이슈 종료·순차 진행을 에이전트에 위임했다. 검증·코드 리뷰 후 병합하며 사용자에게 매번 승인을 다시 요구하지 않는다. 자동 승인 검토 거절은 우회하지 않고 최신 권한 근거로 처리한다. 기술 스파이크는 사용자 승인 범위와 `spikes/` 위치를 확인한다.

## 시작할 때

1. [README](README.md), [검토 체크리스트](docs/design/00-review-checklist.md), [백로그](docs/workflow/backlog.md)를 읽고 git status/현재 브랜치/관련 이슈를 확인한다.
2. 작업의 PRD 요구 ID·시나리오 ID·설계·ADR과 선행 이슈가 완료되었는지 확인한다.
3. 최신 develop에서 `<type>/<issue-number>-<short-kebab-slug>` 브랜치를 만든다. 사용자의 진행 중 변경을 덮어쓰지 않는다.

## 제품 불변 조건

웹 PC/모바일 전용, 1:1 동일 판 대전·봇·데일리, 자체 광고만, 결제 없음. 집 미니 PC+Docker PostgreSQL+Cloudflare Tunnel, IP/DB/포트 공개 금지. 8언어 en/ko/ja/zh-CN/es/pt-BR/de/fr. 제품 규칙은 Rust 하나, online 정답/seed는 서버만 가진다. 광고는 게임 중 표시하지 않는다. OAuth 전용: Google·Apple·카카오·네이버 필수, provider port로 확장. 비밀번호 미보관; 이메일/실명/사진/전화 등 불필요한 권한·필드 없음. 내부 계정 ID·게임용 nickname·provider subject digest만; Apple 철회 credential 예외는 설계를 따른다. 무계정 로컬 연습은 서버 계정을 만들지 않는다.

## 설계와 구현

한국어 대화/문서, 영어 식별자/코드 주석. SRP/SOLID·DIP, Rust struct/trait와 TS interface/DI, 도메인은 인프라를 모른다. Atomic Design+공통 디자인 토큰, 프로토콜 Rust 원천→생성 TS, 중복 규칙 금지. 설계 변경은 코드보다 먼저 문서/ADR/추적표에 반영한다. 의존성 추가는 cargo add / pnpm add 등 버전 인자 없이 최신 안정 버전을 받는다. 설치된 범위와 lockfile은 커밋하고 CI는 locked/frozen 설치를 사용한다. 호환성 문제는 실제 확인해서 해결하며 임의 구버전 pin으로 우회하지 않는다.

승인 후 모든 제품 변경은 시나리오→Red 실행 확인→Green→Refactor→관련 검증이다. 커버리지 목표 코어/도메인95%, 전체80%. 공정성·secret DTO·입력 idempotency·실DB·WS·오프라인·언어·광고 무차단의 의미 있는 행동을 검증한다. 미실행 테스트나 목표 성능을 달성했다고 보고하지 않는다.

## 이슈·PR 종료 조건

작업마다 milestone+issue, PR base는 develop. 초기 설정 요청은 이 범위의 commit/push/PR 생성 권한을 포함한다. 이후 사용자가 맡긴 이슈의 정상 완료 절차도 commit/push/PR 제출을 포함하되 최신 지시를 따른다. **사용자가 순차 전체 작업과 병합을 위임했다.** 에이전트 코드/diff 리뷰→checks 통과→최신 head 확인→squash merge→작업 이슈 종료 확인→다음 이슈 순서다. 형식적인 자기 Approve 리뷰는 만들지 않는다. OAuth 키/도메인/광고 계정 등 외부 입력만 후속 이슈로 남기고 독립적인 개발은 계속한다.

develop이 기본 브랜치다. main은 선택적 release 대상이고 삭제·reset하지 않는다. GitHub는 develop 아닌 기본 브랜치의 이슈 자동종료를 기대하지 않도록 [병합 후 확인](docs/workflow/git-workflow.md)을 따른다. 외부 계정 가입·도메인 구매·Cloudflare/광고 운영자 설정은 절차를 작성하고 사용자가 한다.

## 프로젝트 스킬

- [liar-design-review](.agents/skills/liar-design-review/SKILL.md): 설계/공정성/추적표 리뷰.
- [liar-tdd](.agents/skills/liar-tdd/SKILL.md): 승인 뒤 제품 이슈의 행동 중심 TDD.
- [liar-issue-pr](.agents/skills/liar-issue-pr/SKILL.md): 이슈 시작부터 검토 PR·병합 후 정리.

## 검사와 보고

`python scripts/validate_docs.py`와 [Mermaid 검사 절차](CONTRIBUTING.md)를 실행한다. 제품 검사 명령은 #4에서 확정한다. 아직 수행하지 않은 cargo/pnpm/Playwright 검사를 통과했다고 주장하지 않는다. 비밀값·사용자 파일·보존 원문을 커밋에서 누락/노출하지 않게 diff를 검토한다.
