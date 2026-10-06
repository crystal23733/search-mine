# Liar Sweeper · search-mine

상대 숫자를 속이고, 논리로 간파해 반격하는 웹 1:1 지뢰찾기 프로젝트입니다. PC·모바일, 게스트, 봇·친구 대전, 데일리와 8언어 지원을 설계합니다.

**현재 상태: 기획·설계 문서와 초기 작업 체계 완료, 사용자 설계 승인 대기. 제품은 아직 구현되지 않았습니다.** 제공된 원문은 [docs/planning](docs/planning/HANDOFF_PROMPT.md)에 보존했고 기존 Next/Nest 프로젝트는 확인을 받아 제거했습니다.

## 먼저 읽을 문서

- [PRD: 범위와 인수 조건](docs/product/16-PRD.md)
- [설계 검토·승인 항목·요구 추적표](docs/design/00-review-checklist.md)
- [전체 문서 안내](docs/README.md)
- [마일스톤·작업 이슈](docs/workflow/backlog.md)
- [Git·PR 규칙](docs/workflow/git-workflow.md)
- [기여와 문서 검사](CONTRIBUTING.md)

## MVP와 기술 선택

4분 동일 판 대전, 노게스 생성과 간파 가능한 숫자 공격, 지목 반사, 10초 봇 백필, 친구 코드, 30초 튜토리얼, UTC 데일리·공유·순위, 캐시된 오프라인 연습을 제공합니다. en/ko/ja/zh-CN/es/pt-BR/de/fr로 시작합니다. 공정성·재미·성능은 아직 실측 전이며 검증되지 않은 약속으로 출시하지 않습니다.

Rust 공유 코어(native/WASM), axum/tokio 서버, TypeScript+Vite+Preact/PixiJS, PostgreSQL/sqlx를 사용합니다. 집 미니 PC Docker Compose를 Cloudflare Tunnel로 공개하고 DB·집 IP·서비스 포트를 외부에 노출하지 않습니다. 자체 사이트 광고만 사용하며 플레이 도중 광고를 표시하지 않습니다.

## 작업 흐름

기본 브랜치는 **develop**입니다. milestone+issue→`type/issue-short-slug`→TDD/검증→develop PR→사용자 리뷰→승인 후 squash merge→이슈 종료 확인→다음 작업 순서입니다. 현재 브랜치는 `chore/2-project-foundation`이며 M0 초기 설정 작업입니다. main은 선택적 릴리스용으로 보존합니다.

[AGENTS.md](AGENTS.md)는 공통 에이전트 규칙, [CLAUDE.md](CLAUDE.md)는 Claude 진입점입니다. `.agents/skills/`에 설계 검토·TDD·이슈/PR 스킬을 두고 `.claude/`에서 공유합니다. 적용한 pm-skills와 출처는 [skill-usage](docs/workflow/skill-usage.md)에 기록했습니다.

## 문서 검사

```powershell
python scripts/validate_docs.py
npm install --prefix .tmp/docs-tools --no-audit --no-fund mermaid@11.12.2 jsdom@26.1.0
node scripts/check_mermaid.mjs .tmp/docs-tools
```

GitHub Actions는 문서·Mermaid와 브랜치/이슈/승인 게이트를 검사합니다. 제품 실행·빌드·테스트 명령은 설계 승인 뒤 첫 구현 이슈에서 추가합니다. 서버 사양·실측 요금·도메인·Google 광고 승인·정책 운영자 정보는 공개 전에 사용자가 확인합니다.
