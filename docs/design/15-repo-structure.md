# 15. 모노레포·빌드 경계

> 대응: NFR01/05/08 · **아래 제품 디렉토리는 승인 뒤 생성한다.** 기존 Next/Nest 코드는 재사용하지 않는다.

```text
search-mine/
  AGENTS.md, CLAUDE.md, README.md, CONTRIBUTING.md
  docs/{planning,product,design,adr,workflow}/
  .agents/skills/{liar-design-review,liar-tdd,liar-issue-pr}/
  .claude/{rules,skills}/
  .github/{ISSUE_TEMPLATE,workflows}/
  scripts/{validate_docs.py,check_mermaid.mjs}
  # M1 이후 계획
  Cargo.toml, Cargo.lock
  crates/
    core/           # domain, solver, generator, rules, bot
    protocol/       # public DTO, version, TS generation
    server/         # application + ports + infra + HTTP/WS
    wasm/           # local core boundary
  apps/web/         # Preact atomic UI, Pixi, services, locales
  packages/
    protocol/       # generated TS; 직접 수정 금지
    design-tokens/  # token source and generated CSS/Pixi values
  config/game-rules.toml
  deploy/           # compose/nginx/tunnel/backup/runbook
  tests/{fixtures,e2e,load}/
```

Rust workspace와 pnpm workspace를 사용한다. 사용자 지시에 따라 cargo add / pnpm add에는 버전 인자를 붙이지 않고 최신 안정 릴리스를 추가한다. manifest 자동 생성 범위와 lockfile로 실제 해결된 버전을 기록하고 CI는 locked/frozen 설치한다. 최신 릴리스 간 호환성은 검사하며 필요한 toolchain을 갱신한다. workspace root lockfiles를 커밋한다. 제품 파일은 설계 승인 전 생성하지 않는다. 문서 검사 스크립트·GitHub 템플릿·CI는 초기 작업 체계이므로 이번 PR에 포함한다.

```mermaid
flowchart LR
  Core[Rust core] --> Native[server native]
  Core --> Wasm[wasm-bindgen local core]
  DTO[Rust public protocol] --> Types[ts-rs generated TS]
  Types --> Web[Preact and PixiJS]
  Wasm --> Web
  Tokens[design token source] --> CSS[CSS vars and Pixi values]
  CSS --> Web
  Web --> Assets[Vite static build]
  Native --> Image[server Docker image]
  Assets --> Nginx[nginx static image]
```

TS protocol generator는 schema 변경 PR에서 실행하고 generated diff를 검사한다. 내부 Board/seed 타입은 생성 대상에 넣지 않는다. 버전 불일치·누락은 build fail, Rust native/WASM golden fixtures를 공통으로 읽는다. 규칙은 core 하나이며 i18n·광고·렌더링은 web adapter에 둔다.

문서 원문은 docs/planning에 보존한다. 최신 사용자 지시→AGENTS/워크플로→승인 PRD/설계/ADR→원문 순으로 판단하며 오래된 HANDOFF의 커밋/브랜치 규칙은 최신 workflow가 대체한다.

#101 FR14/16·NFR01/02/05/07/08·TS15/16/20/21/31/35/36 → [ADR0046](../adr/0046-active-match-journal.md) → typed active journal/register/discard/strict final·same-owner startup unknown abort·최소 schema/FK삭제·admitted anchor/seed 상한·실제 PG/관련 gates → [검증101](../verification/101-active-match-journal.md). 공개 admission/main 전환·OS kill-restart 통합은 후속이며 storage foundation을 사용자 복구 완료로 보고하지 않는다.


#103 [ADR0047](../adr/0047-durable-lobby-admission.md): core PreparedEngine의 실제 적용 hash getter, lobby의 명시 LobbyMatchServices/필수 AdmissionJournal 및 receipt 수명을 분리한다. main/HTTPS의 동일 PgJournalRuntime과 실프로세스 복구 검사는 [검증103](../verification/103-durable-lobby-admission.md)을 따른다.
