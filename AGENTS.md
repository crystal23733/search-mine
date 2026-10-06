# Liar Sweeper 작업 지침

## 현재 단계와 우선순위

이 저장소는 기획·설계 리뷰 단계다. 최신 사용자 지시가 우선이고, 승인된 PRD/설계/ADR→이 문서와 workflow→docs/planning 원문 순으로 해석한다. 원문의 Next/Nest·스팀·포털·Cloudflare DO 아이디어는 채택하지 않았다. 오래된 HANDOFF의 브랜치/커밋 관례는 [새 workflow](docs/workflow/git-workflow.md)가 대체한다.

`docs/workflow/design-approval.json`은 현재 pending이다. **사용자의 명시적 설계 승인을 기록하기 전 제품 코드·테스트·패키지·Docker 구성·스파이크를 생성하지 않는다.** 문서 검사·GitHub 템플릿·작업 자동화는 이번 초기 설정 범위다. 기술 스파이크는 별도 사용자 승인과 `spikes/` 위치가 필요하다.

## 시작할 때

1. [README](README.md), [검토 체크리스트](docs/design/00-review-checklist.md), [백로그](docs/workflow/backlog.md)를 읽고 git status/현재 브랜치/관련 이슈를 확인한다.
2. 작업의 PRD 요구 ID·시나리오 ID·설계·ADR과 선행 이슈가 완료되었는지 확인한다.
3. 최신 develop에서 `<type>/<issue-number>-<short-kebab-slug>` 브랜치를 만든다. 사용자의 진행 중 변경을 덮어쓰지 않는다.

## 제품 불변 조건

웹 PC/모바일 전용, 1:1 동일 판 대전·봇·데일리, 자체 광고만, 결제 없음. 집 미니 PC+Docker PostgreSQL+Cloudflare Tunnel, IP/DB/포트 공개 금지. 8언어 en/ko/ja/zh-CN/es/pt-BR/de/fr. 제품 규칙은 Rust 하나, online 정답/seed는 서버만 가진다. 광고는 게임 중 표시하지 않는다.

## 설계와 구현

한국어 대화/문서, 영어 식별자/코드 주석. SRP/SOLID·DIP, Rust struct/trait와 TS interface/DI, 도메인은 인프라를 모른다. Atomic Design+공통 디자인 토큰, 프로토콜 Rust 원천→생성 TS, 중복 규칙 금지. 설계 변경은 코드보다 먼저 문서/ADR/추적표에 반영한다.

승인 후 모든 제품 변경은 시나리오→Red 실행 확인→Green→Refactor→관련 검증이다. 커버리지 목표 코어/도메인95%, 전체80%. 공정성·secret DTO·입력 idempotency·실DB·WS·오프라인·언어·광고 무차단의 의미 있는 행동을 검증한다. 미실행 테스트나 목표 성능을 달성했다고 보고하지 않는다.

## 이슈·PR 종료 조건

작업마다 milestone+issue, PR base는 develop. 초기 설정 요청은 이 범위의 commit/push/PR 생성 권한을 포함한다. 이후 사용자가 맡긴 이슈의 정상 완료 절차도 commit/push/PR 제출을 포함하되 최신 지시를 따른다. **리뷰 없는 자동 병합·자동 다음 기능 착수는 하지 않는다.** 사용자 리뷰/명시 병합 승인→checks 통과→squash merge→작업 이슈 닫기→다음 이슈 순서다.

develop이 기본 브랜치다. main은 선택적 release 대상이고 삭제·reset하지 않는다. GitHub는 develop 아닌 기본 브랜치의 이슈 자동종료를 기대하지 않도록 [병합 후 확인](docs/workflow/git-workflow.md)을 따른다. 외부 계정 가입·도메인 구매·Cloudflare/광고 운영자 설정은 절차를 작성하고 사용자가 한다.

## 프로젝트 스킬

- [liar-design-review](.agents/skills/liar-design-review/SKILL.md): 설계/공정성/추적표 리뷰.
- [liar-tdd](.agents/skills/liar-tdd/SKILL.md): 승인 뒤 제품 이슈의 행동 중심 TDD.
- [liar-issue-pr](.agents/skills/liar-issue-pr/SKILL.md): 이슈 시작부터 검토 PR·병합 후 정리.

## 이번 단계 검사

`python scripts/validate_docs.py`와 [Mermaid 검사 절차](CONTRIBUTING.md)를 실행한다. 제품이 없으므로 cargo/pnpm/Playwright 검사를 이번 PR에서 통과했다고 주장하지 않는다. 비밀값·사용자 파일·보존 원문을 커밋에서 누락/노출하지 않게 diff를 검토한다.
