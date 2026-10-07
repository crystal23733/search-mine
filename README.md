# Liar Sweeper · search-mine

상대 숫자를 속이고, 논리로 간파해 반격하는 웹 1:1 지뢰찾기 프로젝트입니다. PC·모바일, 최소 정보 OAuth 계정, 봇·친구 대전, 데일리와 8언어 지원을 설계합니다.

**현재 상태: M0/M1과 #4~10 병합 완료. #11 공개 Pixi 보드·터치·키보드·DOM 접근성을 구현·검증했습니다. #12 실제 로컬 대전·튜토리얼과 OAuth 연동을 순차 진행합니다.** [거짓말 검증 결과와 제한](docs/verification/06-lie-certification.md)·[규칙](docs/verification/07-rule-engine.md)·[봇 검증](docs/verification/08-public-bots.md)·[WASM 검증](docs/verification/09-public-wasm.md)·[웹 shell](docs/verification/10-atomic-shell.md)·[보드 검증과 화면](docs/verification/11-public-board.md)·[실제 로컬 대전/학습](docs/verification/12-local-practice-tutorial.md)을 확인하세요. 제공된 원문은 [docs/planning](docs/planning/HANDOFF_PROMPT.md)에 보존했고 기존 Next/Nest 프로젝트는 확인을 받아 제거했습니다.

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

## OAuth와 화면 기준

Google·Apple·카카오·네이버는 첫 공개 버전 필수이며 다른 제공자는 어댑터로 추가합니다. 비밀번호를 저장하지 않고 최소 로그인 식별자와 게임용 닉네임만 요구합니다. 이메일·실명·사진·전화 권한은 요청하지 않습니다. Apple 계정 삭제용 credential은 서버에 암호화 보관합니다. 로그인 없이 로컬 봇/튜토리얼/데일리 연습을 제공하고 온라인 대전·공식 기록은 인증합니다.

[인증/개인정보 설계](docs/design/17-auth-privacy.md), [18개 디자인 화면 기준](docs/design/18-design-layout.md), [사용자 지시 기록](docs/workflow/approval-record.md)을 확인합니다. 제공된 design_layout 원본을 보존합니다.

## 작업 흐름

기본 브랜치는 **develop**입니다. milestone+issue→`type/issue-short-slug`→TDD/검증→develop PR→에이전트 리뷰·검사→위임에 따라 squash merge→이슈 종료 확인→다음 작업 순서입니다. #2~11은 병합·종료됐고 #12 로컬 대전/학습을 검증했습니다. 일별 코드 리뷰는 Git에서 제외한 로컬 `review/YYYY-MM-DD.md`에 기록합니다. main은 선택적 릴리스용으로 보존합니다.

[AGENTS.md](AGENTS.md)는 공통 에이전트 규칙, [CLAUDE.md](CLAUDE.md)는 Claude 진입점입니다. `.agents/skills/`에 설계 검토·TDD·이슈/PR 스킬을 두고 `.claude/`에서 공유합니다. 적용한 pm-skills와 출처는 [skill-usage](docs/workflow/skill-usage.md)에 기록했습니다.

## 개발 실행

Node.js 24.15 이상, 최신 Rust stable, pnpm을 사용합니다. 의존성 추가에는 버전을 붙이지 않고 최신 안정 릴리스를 받습니다. 설치 결과는 Cargo.lock/pnpm-lock.yaml에 기록하고 검증/CI는 locked/frozen으로 재현합니다.

```powershell
npm install --global pnpm
pnpm install --frozen-lockfile
rustup update stable
rustup target add wasm32-unknown-unknown
cargo install wasm-bindgen-cli --locked
pnpm types:check
pnpm wasm:build
pnpm dev
# 다른 터미널: health 서버 (기본 127.0.0.1:3000)
cargo run --locked -p liar-server
```

현재 웹은 제공된 디자인의 홈·규칙·언어·접근성 설정을 제공합니다. `/ko/`, `/en/` 등8언어 URL을 사용하며 게임·계정·정책 연결은 후속 이슈에서 진행합니다. 첫 방문은 튜토리얼을 시작하고 skip/replay할 수 있습니다. `/ko/practice?difficulty=easy`의 난이도는 easy/normal/hard이며 계정을 만들지 않습니다. 실제 사람의 30초 이해·재미는 #27 관찰 전까지 미확인입니다. 아직 로그인 제공자 키를 요구하지 않으며 #15에서 실제 연동 준비 항목을 별도 기록합니다. 디자인 토큰 변경은 `pnpm tokens:generate`, 형식 검사는 `pnpm format:check`로 실행합니다.

## 검증

```powershell
pnpm exec playwright install chromium
./scripts/check.ps1
# 실제 DB 통합 (개발용 loopback DB, 운영 DB는 host 포트 없음)
docker compose -f tests/compose.postgres.yml up -d --wait
$env:DATABASE_URL = 'postgres://liar_test:development-only@127.0.0.1:54329/liar_test'
cargo test --locked -p liar-server --test postgres -- --ignored
docker compose -f tests/compose.postgres.yml down
```

`pnpm docs:check`, `pnpm lint/typecheck/test/build/test:e2e`는 각 script를 개별 실행합니다. `pnpm build`는 공개 WASM도 생성하며 `wasm:generate`/`fixtures:generate`는 Rust 계약 변경 시 생성 선언/재생 fixture를 갱신합니다. CI는 freshness와 실제 Chromium 재생을 검사합니다. GitHub Actions의 docs/contribution-gate/rust/web/database/coverage를 모두 확인한 뒤 develop에 병합합니다. DB 없는 기본 cargo test에서 무시되는 PostgreSQL 테스트를 통과로 계산하지 않습니다.

서버 사양·실측 요금·도메인·OAuth 키·광고/CMP 승인·정책 운영자 정보는 [외부 준비 목록](docs/workflow/external-inputs.md)에 기록합니다. 필요한 입력을 만들지 않고 독립적인 개발은 계속합니다.

코어는 거짓말 없는 생성 인증과 온라인 간파 검증을 분리합니다. [#5 검증 결과](docs/verification/05-generation-solver.md)는 작은 판 전수·seed corpus·개발 PC release 벤치의 실제 근거입니다. 아직 #6 공정성이나 미니 PC 성능 검증을 대신하지 않습니다.

```powershell
cargo run --locked --release -p liar-core --example generate_bench -- 10000 .tmp/generation.csv
rustup component add llvm-tools-preview
cargo install cargo-llvm-cov --locked
cargo llvm-cov --locked -p liar-core --tests --ignore-filename-regex '[/\\](tests|examples)[/\\]' --fail-under-lines 95
```
