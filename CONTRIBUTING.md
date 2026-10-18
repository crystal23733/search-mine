# 기여와 검증

최신 develop에서 milestone/issue가 있는 `type/issue-kebab-slug` 브랜치를 만든다. [워크플로](docs/workflow/git-workflow.md)를 따른다. 사용자 위임으로 에이전트가 실제 코드 리뷰·검사·squash merge·이슈 종료·다음 작업을 진행한다.

## 개발 환경

Node≥24.15, 최신 Rust stable, Yarn 4. `corepack enable`과 `yarn install --immutable`로 설치하며 `yarn --version`으로 packageManager와 일치하는지 확인한다. Corepack이 없는 Node 배포판은 먼저 `npm install --global corepack`으로 설치한다. 새 의존성은 `cargo add 패키지` / `yarn workspace @liar/web add 패키지` 등 버전 인자 없이 최신 안정 릴리스를 받는다. 해결된 범위와 lockfile을 함께 커밋하고 peer/MSRV/API 호환성을 실제 확인한다. workspace 추가는 루트 package.json, 빌드 허용과 선언 패치는 [ADR0078](docs/adr/0078-yarn-workspace.md)을 따른다.

## 검증 명령

`./scripts/check.ps1`은 fmt/clippy/Rust 단위/WASM 컴파일/생성 타입/TS lint·typecheck·test·build/Playwright/문서·Mermaid를 순서대로 실행하며 실패 즉시 종료한다. Playwright는 먼저 `yarn playwright install chromium`으로 설치한다.

실DB 검사는 README의 개발 Compose와 DATABASE_URL을 준비해 `cargo test --locked -p liar-server --test postgres -- --ignored`로 별도 실행한다. GitHub database job은 실제 PostgreSQL18에서 반드시 수행한다. 로컬 Docker 사용 불가는 미실행으로 보고하고 CI 결과를 따로 기록한다.

문서만 바꿨으면 `yarn docs:check`와 `git diff --check`를 수행한다. generated TS는 직접 수정하지 않고 `yarn types:generate`로 만든 뒤 `yarn types:check`를 실행한다. 기획/설계/ADR/시나리오는 제품 코드보다 먼저 갱신한다.

## PR 기록

관찰 가능한 Red 실패→Green 결과·변경 이유·실제 코드 리뷰 결과·관련 TS/요구 ID·미실행 외부 환경을 적는다. 각 이슈가 완료된 뒤 develop 대상 PR을 만들고 필수 검사를 확인하여 병합한다. OAuth 키·광고 계정·정책 운영자 값은 만들어 넣지 않고 후속 사용자 입력 항목으로 남긴다. 미구현 기능과 커버리지/성능/공정성 목표를 달성한 것으로 보고하지 않는다.
