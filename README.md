# Liar Sweeper · search-mine

상대 숫자를 속이고, 논리로 간파해 반격하는 웹 1:1 지뢰찾기 프로젝트입니다. PC·모바일, 최소 정보 OAuth 계정, 봇·친구 대전, 데일리와 8언어 지원을 설계합니다.

**현재 상태: M0/M1과 #4~14·#43~46·#16~17 병합 완료. 빠른/친구 대전·실제 WS 보드/결과를 연결했고 #18 재접속·장애 복구를 순차 구현합니다.** [계정 화면·실제 HTTPS 검증](docs/verification/46-oauth-account-ui.md)을 확인하세요. [거짓말 검증 결과와 제한](docs/verification/06-lie-certification.md)·[규칙](docs/verification/07-rule-engine.md)·[봇 검증](docs/verification/08-public-bots.md)·[WASM 검증](docs/verification/09-public-wasm.md)·[웹 shell](docs/verification/10-atomic-shell.md)·[보드 검증과 화면](docs/verification/11-public-board.md)·[실제 로컬 대전/학습](docs/verification/12-local-practice-tutorial.md)·[데일리/기록/공유](docs/verification/13-utc-daily-records-share.md)·[오프라인/업데이트/대기](docs/verification/14-public-offline-cache.md)을 확인하세요. 제공된 원문은 [docs/planning](docs/planning/HANDOFF_PROMPT.md)에 보존했고 기존 Next/Nest 프로젝트는 확인을 받아 제거했습니다.

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

기본 브랜치는 **develop**입니다. milestone+issue→`type/issue-short-slug`→TDD/검증→develop PR→에이전트 리뷰·검사→위임에 따라 squash merge→이슈 종료 확인→다음 작업 순서입니다. #2~14는 병합·종료됐습니다. OAuth는 #43→44→45→46으로 순차 구현하고 #42 실제 계정 검수는 별도 출시 조건입니다. 일별 코드 리뷰는 Git에서 제외한 로컬 `review/YYYY-MM-DD.md`에 기록합니다. main은 선택적 릴리스용으로 보존합니다.

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

현재 웹은 제공된 디자인의 홈·규칙·언어·접근성 설정·실제 로컬 봇/학습·UTC 데일리를 제공합니다. `/ko/`, `/en/` 등8언어 URL을 사용하며 OAuth 계정·빠른/친구 온라인 대전 화면을 제공합니다. 재접속·장애 복구와 정책 연결은 후속 이슈에서 진행합니다. 첫 방문은 튜토리얼을 시작하고 skip/replay할 수 있습니다. `/ko/practice?difficulty=easy`의 난이도는 easy/normal/hard이며 계정을 만들지 않습니다. 실제 사람의 30초 이해·재미는 #27 관찰 전까지 미확인입니다. 서버 OAuth 설정은 준비됐으며 실제 키·제공자 등록/실계정 검수는 #42 외부 준비 항목입니다. 디자인 토큰 변경은 `pnpm tokens:generate`, 형식 검사는 `pnpm format:check`로 실행합니다.

## 오프라인 확인

`pnpm build` 뒤 `pnpm --filter @liar/web preview --port 4173`으로 production을 연다. 설정의 캐시 준비를 확인한 뒤 연결을 끊으면 저장된8언어·WASM·로컬 봇/데일리를 이용할 수 있다. 개발 서버5173에서는 SW를 켜지 않는다. 최초 미캐시 offline 방문은 지원하지 않으며 브라우저가 캐시를 지울 수 있다. 업데이트는 열린 모든 탭이 홈으로 돌아온 뒤 버튼으로 적용한다. 설정에서 공개 cache와 개인 기록/제출 대기를 각각 삭제할 수 있다. 제출 후보는 미검증이며 실제 로그인은 #46에서 연결했으며 공식 전송은 #19에서 연결한다.

## 검증

[OAuth 운영 설정](docs/workflow/oauth-configuration.md)은 키 없는 비활성 실행·secret 변수·최소 callback/권한과 실제 계정 검수의 한계를 기록합니다. 실계정 검수 #42를 테스트 fixture 성공으로 대체하지 않습니다.

인증 기반과 실DB/coverage 결과는 [#43 검증](docs/verification/43-auth-foundation.md)을 확인합니다. 네 제공자와 로그인 HTTP는 [#44 검증](docs/verification/44-oauth-providers-http.md)에 기록합니다. 계정 권리는 [#45 검증](docs/verification/45-account-rights-apple-revocation.md)에 기록하며 계정 화면은 [#46 검증](docs/verification/46-oauth-account-ui.md)을 확인합니다. Migration은 serving 시작과 별도로 실행하며 운영에서는 DDL 계정과 DML 계정을 분리합니다. 아래는 격리된 테스트 DB 예시입니다.

```powershell
pnpm exec playwright install chromium
./scripts/check.ps1
# 실제 DB 통합 (개발용 loopback DB, 운영 DB는 host 포트 없음)
docker compose -f tests/compose.postgres.yml up -d --wait
$env:DATABASE_URL = 'postgres://liar_test:development-only@127.0.0.1:54329/liar_test'
$env:MIGRATION_DATABASE_URL = $env:DATABASE_URL
cargo run --locked -p liar-server --bin migrate
Remove-Item Env:MIGRATION_DATABASE_URL
cargo test --locked -p liar-server --test postgres -- --ignored
# 실제 DB가 실행된 환경에서 인증 coverage (entry point 포함)
cargo llvm-cov --locked -p liar-server --tests --ignore-filename-regex '[/\\](tests|examples)[/\\]' --json --output-path .tmp/auth-coverage.json -- --include-ignored
python scripts/check_auth_coverage.py .tmp/auth-coverage.json
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

## 계정 화면 검증

[#66 OAuth 초대 복귀](docs/verification/66-oauth-invitation-return.md)는 `/{locale}/login?return_path=friends&code=ABCD2345`의 canonical 초대를 서버5분 인증 거래에 보존한다. callback·nickname·tutorial 뒤 같은 code로 friends에 돌아가고, 소비된 취소/실패에서도 안전한 새 로그인 재시도를 제공한다. 실제 로비/온라인 화면은 #70/#72에서 연결했다.

`/{locale}/login`, `/onboarding`, `/settings`에서 네 제공자·닉네임·연결/해제·내보내기·로그아웃·삭제를 제공한다. 실제 제공자 키/등록/계정 확인은 #42이며, 키가 없는 실행에서도 무계정 연습은 가능하다. 공식 제출은 #19이며 친구 초대의 실제 OAuth 왕복은 #66/#72에서 검증했다.

위의 격리된 PostgreSQL 테스트 DB가 실행되고 `DATABASE_URL`이 설정된 환경에서 아래를 실행한다. 실제 제품 router와 DB를 사용하고 외부 제공자 proof만 시험 port로 대체한다. 저장소의 HTTPS 키는 공개된 loopback 시험 전용이며 운영 키로 사용하지 않는다.

```powershell
$env:MIGRATION_DATABASE_URL = $env:DATABASE_URL
cargo run --locked -p liar-server --bin migrate
Remove-Item Env:MIGRATION_DATABASE_URL
pnpm build
pnpm exec playwright test --project auth
```

DB 환경변수가 없는 기본 로컬 실행은 browser50개만 검사하고 auth project는 실행하지 않는다. CI는 별도 PostgreSQL18을 실행하여 HTTPS 계정·8언어 초대·빠른/친구 대전·실제 WS·취소·철회·재접속 시험을 포함한85개를 실행한다. [#70 빠른 대전](docs/verification/70-online-quick-match-ui.md)과 [#72 친구 방 검증](docs/verification/72-friends-room-ui.md)을 확인한다. [#85 업데이트 진단](docs/verification/85-offline-update-observation.md)은 기존 SW 간헐 실패 #83의 trace를 보존하며 원인 해결을 뜻하지 않는다.

단위 테스트는 순수 포트의 Node 환경과 실제 DOM의 jsdom 환경을 구분한다. [#84 준비·DOM 조회 검증](docs/verification/84-unit-arrangement-observation.md)은 기존 시간 제한과 접근성 단언을 유지한 실행 결과와 한계를 기록한다.

[#68 세션 인증 요청 포트](docs/verification/68-authenticated-web-requests.md)는 온라인 조회의 전/후 bootstrap·fresh CSRF와 취소/late reply를 제공한다. 실제 로비/WS 화면은 #70/#72에서 연결했다. #74 재접속 입력 순번 계약은 ADR0035를 따른다.

[#76 상대 재연결 유예](docs/verification/76-opponent-reconnect-grace.md)는 서버가 계산한 상대의 연결 유예를8언어로 보드 위에 표시한다. 표시가0이어도 서버 결과를 기다리고, 상대가 복귀하면 같은 판을 이어간다. 본인 자동 재연결과 서버 재시작 복구는 #18 후속 작업이다.

[#78 세션 복구 후보](docs/verification/78-session-recovery-candidate.md)는 offline 시 공개 계정/요청 권한을 차단하고 최대30초 메모리 후보만 보존한다. 원래 account/session의 fresh proof에 성공해야 복원되며 중단 전 데일리 제출의 늦은 응답은 기록을 지우지 않는다. WS 자동 backoff·미확인 입력 재전송은 다음 #18 하위 작업이다.

## 권위 서버와 WebSocket

[#82 본인 완료 결과 조회](docs/verification/82-authenticated-personal-result-read.md)는 `POST /api/v1/results`에 `{v:1,match_id}`를 보내 저장된 본인 결과만 조회한다. 인증 쿠키·Origin·CSRF·닉네임이 필요하며 90일 미만의 본인 참여 결과만 반환한다. 상대 식별자/통계·seed·보드·현재 rules는 없다. 조회 전후 세션을 확인하고 철회 중 응답을 차단한다. 없는/타 계정/삭제/만료 결과의 같은404를 진행 상태나 승패로 추정하지 않는다. 웹 소비·서버 재시작 복구는 후속 #18이다. `LIAR_RESULT_AUTHORITIES=64`, `LIAR_RESULT_REQUESTS=16`은 개발 기본값이고 각각1..20000/1..256만 허용한다. 전체 IO2초·session/account 초당20회 상한은 #26의 실측을 대신하지 않는다.

[#80 제한된 자동 재연결](docs/verification/80-bounded-online-reconnect.md)은 끊김 동안 읽기 전용 보드를 유지하고 최대30초 자동 복구를 시도한다. 같은 세션의 새 snapshot 뒤 메모리 미확인 명령을 같은 UUID로 재전송한다. 서버 재시작 복구·결과 조회는 후속 #18이다.

[#16 검증과 제한](docs/verification/16-authoritative-match-actor.md)을 확인한다. 인증 설정을 갖춘 서버는 `/api/v1/ws`에 정확한 Origin·서비스 쿠키·닉네임이 있는 계정만 허용한다. `POST /api/v1/lobby`의 인증 큐/친구방에서 배정된 계정이 게임 WS를 연결한다. 서버가 seed/정답을 소유하며 클라이언트에 공개 projection만 보낸다. 웹의 로비/온라인 화면은 #70/#72에서 연결했고 자동 재접속은 #18의 후속 작업이다.

개발 제한의 기본값은 매치16/mailbox64/송신16/proof2/물리 연결32이며 `LIAR_ONLINE_MATCHES`, `LIAR_ONLINE_MAILBOX`, `LIAR_ONLINE_OUTGOING`, `LIAR_ONLINE_PROOF_WORKERS`, `LIAR_ONLINE_CONNECTIONS`로 설정한다. 잘못된 값은 시작을 거절한다. 이 값은 미니 PC 출시 성능의 실측 결과가 아니며 #26에서 admission을 결정한다. 최종 결과는 새 migration으로 저장하고 닉네임·세션을 복사하지 않는다. 계정 삭제는 참여 기록을 제거하며 retry로 복원하지 않는다.

매칭의 [원자 큐·친구방 정책](docs/verification/52-atomic-lobby-policy.md)과 [비공개 인증 보드 공급 포트](docs/verification/54-private-certified-board-pool.md)를 준비했다. 보드 풀은 생성/저장 수와 대기를 제한하며 게임 countdown은 보드 소비 뒤 시작한다. 인증 로비/초대 화면은 #64/#66/#70/#72에서 연결했다.

[온라인 봇 driver](docs/verification/56-public-observation-online-bot.md)는 공개 Projection과 독립 행동 RNG만으로 core의3난이도를 실행하고 기존 actor에서 입력·결과를 직렬 적용한다. 생성 CPU와 달리 봇 계산도 별도 worker 상한을 가지며 늦은 결과/내부 실패를 처리한다. 공개 로비의 인증 전송·화면/main은 #64/#70에서 연결했다.

[실제 매치 조립](docs/verification/58-atomic-lobby-match-composition.md)의 내부 LobbyService는 큐/친구방 예약→제한된 준비→registry/core bot을 연결한다. core의 준비와 시작을 나누어 준비 완료 시각부터 전체 countdown을 받고, 취소/만료된 generation의 결과를 버린다. 공개 로비의 인증 전송·초대 OAuth·화면/main은 #64/#66/#70/#72에서 연결했다.

[로비 세션 권한](docs/verification/60-session-bound-lobby-admission.md)은 별도 공유 lease와 합성 철회 barrier로 대기/준비 중 logout·삭제·expiry를 적용한다. 두 참여자의 검증과 실제 registry 생성은 같은 authority lock에서 수행하고 유효 상대의 순서/방을 복구한다. 최초 WS·공개 HTTP/main·초대/화면은 #62/#64/#66/#70/#72에서 통합했다.

[최초 게임 연결](docs/verification/62-initial-match-connection-lifecycle.md)은 실제 로비 배정 뒤 actor가 각 인간의 초기 세션 권한을 소유한다. 시작 기한까지 모두 처음 연결하지 않거나 그 전에 철회/만료되면 Rust core의 시작 전 취소로 종료한다. 공개 HTTP/main·초대/8언어 화면은 #64/#66/#70/#72에서 연결했다.

[인증 로비 HTTP와 실행 구성](docs/verification/64-authenticated-lobby-http-runtime.md)은 `POST /api/v1/lobby`에 `{v:1,command:{type:"status"}}` 등 Rust→TS 계약을 제공한다. 모든 호출에 Origin·session-bound CSRF·닉네임을 요구하고 동시 read/2초 deadline·세션/계정 rate·room_join5회/분을 적용한다. main은 실제 비공개 보드/core bot과 두 권한의 합성 철회를 연결한다. OAuth 초대·8언어 로비/온라인 화면은 #66/#70/#72에서 연결했다.

로비 개발 기본값은 `LIAR_LOBBY_CAPACITY=32`, `LIAR_LOBBY_WORKERS=2`, `LIAR_LOBBY_AUTHORITIES=64`, `LIAR_LOBBY_REQUESTS=16`, `LIAR_BOARD_CAPACITY=4`, `LIAR_BOARD_WORKERS=1`, `LIAR_BOT_WORKERS=2`다. 잘못된 구성은 시작을 거절한다. auth 설정이 없으면 로비는 unavailable이며 무계정 연습은 유지한다. 이 상한은 #26의 미니 PC 실측을 대신하지 않는다.

[#89 저장 결과 확인](docs/verification/89-personal-result-web.md)은 연결 복구 실패/저장 알림 유실 후 현재 메모리 판의 본인 결과를 명시 조회한다. 전후 인증 proof와 최소 본인 통계·8언어 화면, 실제 actor 정리 뒤 PG/HTTPS 검증을 포함한다. 새로고침 후 판 발견과 영속 재시작 출구는 후속 #18이다.

[#91 인증 복구 관측](docs/verification/91-auth-recovery-observation.md)은 실제 HTTPS 철회 응답의 관측 수명을 앱의 권한 폐기와 분리해 검사한다. #89/PR90은 병합·종료했으며 CI의 기존 flaky 두 건과 로컬 재시도0 결과를 구분해 기록했다.

[#93 미확인 결과 계약](docs/verification/93-unknown-result-details.md)은 비정상 종료로 최종 수치를 알 수 없을 때 elapsed/본인 통계를 null로 표현한다. 숫자를 추측하지 않으며 실제 재시작 복구는 후속18이다.

[#95 결과 응답 관측](docs/verification/95-result-response-observation.md)은 실제 서버 본문을 앱에 전달하기 전에 관측하여 늦은 CDP 본문 조회 의존을 제거한다. #93/PR94은 병합·종료됐고 최초 CI flaky 기록을 보존했다.
