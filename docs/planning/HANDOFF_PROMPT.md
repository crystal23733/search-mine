# 핸드오프 프롬프트 — "거짓말 지뢰찾기(Liar Sweeper)" 웹게임 기획·설계·구현

> 이 파일 전체를 새 Claude Code 세션의 첫 메시지로 전달한다.
> 실행 예 (PowerShell, 저장소 루트에서):
> `claude (Get-Content docs/planning/HANDOFF_PROMPT.md -Raw)`

---

## 너의 역할

너는 이 저장소(`search-mine`)에서 **1인 개발자(사용자)를 돕는 PM 겸 리드 아키텍트 겸 리드 엔지니어**다. 사용자는 바이브코딩으로 빠르게 진행하길 원한다. 하지만 **코드보다 기획과 설계를 먼저 확실하게 끝내는 것**이 이 프로젝트의 최우선 원칙이다.

진행 순서는 다음과 같다.
1. 저장소 초기화
2. pm-skills로 기획 문서 작성
3. 다이어그램이 포함된 상세 설계서 작성
4. **사용자의 설계 승인**
5. TDD로 구현

먼저 `docs/planning/01-brainstorm.md`를 읽어라. 아이디어의 배경과 시장 조사가 들어 있다. **단, 이 문서의 내용이 brainstorm 문서보다 우선한다.** brainstorm 문서에 있는 스팀, Tauri, CrazyGames/Poki, Cloudflare Durable Objects 관련 내용은 이번 범위에서 **제외**한다.

---

## 확정 사항 (변경 금지, 바꿔야 한다면 사용자에게 먼저 물어볼 것)

1. **장르:** 실시간 1:1 대전 지뢰찾기. 핵심 차별점은 "상대에게 거짓말을 하는 지뢰찾기"다.
2. **플랫폼:** 웹 브라우저 전용(PC와 모바일 모두). 스팀과 외부 웹게임 포털은 범위 밖이다.
3. **수익:** 자체 사이트에 광고를 단다(Google AdSense / H5 Games Ads(Ad Placement API)). 결제 기능은 넣지 않는다.
4. **서버:** **집의 미니 PC 한 대에서 홈서버로 운영**한다. AWS 같은 유료 클라우드 서버는 쓰지 않는다. 외부 공개는 **Cloudflare Tunnel**로 하고, 집 IP는 노출하지 않으며 포트포워딩도 하지 않는다.
5. **DB:** **PostgreSQL**을 쓴다. 홈서버의 Docker 컨테이너로 운영하고, 외부 네트워크에는 노출하지 않는다.
6. **대상:** 전 세계 유저. **다국어(i18n) 지원은 필수**다.
7. **목표:** 손해 보지 않기. 월 운영비는 전기요금과 도메인 비용 수준으로 유지한다.
8. **처음부터 새로 만든다:** 현재 저장소의 기존 프로젝트 파일은 전부 삭제한다(0단계 참고). 기존 코드는 참고하지도 재사용하지도 않는다.
9. **설계 우선:** 1~2단계의 기획·설계 문서와 다이어그램이 완성되고 **사용자가 승인하기 전에는 제품 코드를 한 줄도 작성하지 않는다.** 기술 검증용 스파이크가 필요하면 사용자에게 먼저 묻고, 결과물은 `spikes/`에 두었다가 버린다.

---

## 개발 원칙 (사용자의 개발 스타일, 모든 단계에서 반드시 준수)

### 객체지향(OOP)과 SRP
- 모든 모듈, 클래스, 함수는 **변경 이유가 하나뿐**이도록(SRP) 나눈다. 설계서의 클래스 다이어그램이 이 기준을 그대로 반영해야 한다.
- SOLID 전체를 지킨다. 특히 의존성 역전을 적용해서 도메인은 인프라(DB, 네트워크, 렌더러)를 모르게 하고, 인터페이스로 주입받게 한다.
- **Rust:** 상속이 없으므로 `struct` + `trait`으로 캡슐화와 다형성을 구현한다. 리포지토리, 클럭, 난수 생성기, 트랜스포트는 trait으로 추상화해서 테스트 대역(mock/fake)으로 바꿀 수 있게 한다.
- **TypeScript:** 클래스나 인터페이스 기반으로 서비스(네트워크, i18n, 광고, 저장소)를 분리하고 DI로 조립한다.
- 계층 구조는 도메인 → 애플리케이션(유스케이스) → 인프라 → 프레젠테이션 순으로 둔다(클린 아키텍처 / 헥사고날).

### TDD
- **모든 제품 코드는 Red → Green → Refactor 순서로 작성한다.** 테스트를 먼저 작성하고, 실패하는 것을 확인한 뒤 구현한다.
- 테스트 계층:
  - 단위 테스트: Rust `cargo test` + `proptest`, TS `Vitest`.
  - 통합 테스트: 실제 PostgreSQL(testcontainers 또는 docker compose 테스트 DB)과 WebSocket.
  - E2E: `Playwright`. 대전 1판, 데일리 퍼즐, 언어 변경, 광고 동의 흐름.
  - 부하 테스트: `k6`. 미니 PC에서 동시 대전을 몇 개까지 감당하는지 측정.
- 커버리지 목표: 도메인과 코어 95% 이상, 전체 80% 이상. 숫자 채우기보다 **행동 명세로서의 테스트**를 우선한다.
- 설계 단계에서 `test-scenarios` 스킬로 테스트 시나리오를 먼저 문서화하고, 구현 단계에서 그 시나리오를 테스트 코드로 옮긴다.

### 아토믹 디자인과 재사용, 중복 제거
- UI 컴포넌트는 **Atoms → Molecules → Organisms → Templates → Pages**로 구성한다.
  - 예: Atom = `Button`, `Icon`, `Cell`. Molecule = `LanguageSelect`, `GaugeBar`. Organism = `Board`, `MatchHud`, `ResultCard`. Template = `MatchLayout`. Page = `MatchPage`.
- PixiJS 렌더링 쪽도 같은 원칙으로 재사용 가능한 디스플레이 컴포넌트(셀 스프라이트, 이펙트, 게이지)로 나눈다.
- 디자인 토큰(색, 간격, 타이포, 모션)을 한곳에서 관리한다. 하드코딩한 스타일 값은 금지한다.
- **중복 코드 금지(DRY).** 규칙 로직은 Rust 코어에만 두고, 클라이언트(WASM)와 서버(네이티브)가 같은 코드를 공유한다. 프로토콜 타입은 하나의 스키마에서 Rust와 TS 양쪽으로 생성한다(예: `ts-rs` 또는 `typeshare`).

### 최적화와 성능 (초기 목표치, 설계서에서 확정)
- 클라이언트: 초기 JS 번들 gzip 200KB 이하(WASM 별도), 저사양 모바일에서 60fps, LCP 2.5초 이하.
- 서버: 입력 처리 p95 20ms 이하(서버 내부 기준), 미니 PC 기준 동시 대전 수를 부하 테스트로 측정해서 문서화한다.
- 판 생성: 16×16, 지뢰 40개 노게스 판을 100ms 안에 생성.
- 불필요한 할당, 리렌더, 네트워크 왕복을 줄인다. 최적화는 측정(벤치마크, 프로파일러)을 근거로 한다.

### 보안
- **서버가 권위를 가진다.** 클라이언트는 입력만 보내고, 판의 정답은 클라이언트에 보내지 않는다.
- 모든 입력을 검증하고, 메시지에 레이트 리밋을 걸고, 비정상적인 입력 속도는 감지한다.
- DB 접근은 파라미터 바인딩(sqlx)만 쓰고, 최소 권한 DB 계정을 사용한다.
- 보안 헤더(CSP는 광고 도메인을 허용 목록으로 관리), HTTPS(Cloudflare), CORS를 최소한으로 설정한다.
- 비밀값은 `.env`로 관리하고 커밋하지 않는다. `cargo audit`와 `npm audit`(또는 `pnpm audit`)으로 의존성을 점검한다.
- 설계 단계에서 **STRIDE 위협 모델링** 문서를 작성한다.

---

## 게임 규칙 (MVP, 설계 단계에서 정식 명세로 확정)

### 기본 흐름
- 두 플레이어는 **같은 시드로 만든 똑같은 판**을 **각자** 동시에 푼다.
- 먼저 안전한 칸을 모두 여는 사람이 이긴다. 제한 시간(기본 4분)이 끝나면 진행률이 높은 쪽이 이긴다.
- 지뢰를 밟아도 바로 지지 않는다. **3초 기절**하고, 그 칸은 지뢰로 자동 표시된다.

### 공격: 거짓말
- 안전한 칸을 열 때마다 **공격 게이지**가 찬다.
- 게이지가 차면 상대 판에서 **아직 열리지 않은 숫자 칸 하나**를 골라 숫자를 **±1 거짓**으로 바꾼다. 범위는 1~8이다. 0 칸(연쇄로 열리는 빈 칸)은 대상에서 제외한다.
- 상대가 그 칸을 열면 거짓 숫자가 보인다. 상대는 공격을 받았다는 사실을 알 수 없고, 짧은 경고 연출만 나온다.
- 한 플레이어 판에 동시에 걸려 있을 수 있는 거짓말은 최대 2개다.

### 방어: 지목
- 열린 숫자 칸을 **"거짓말 지목"**할 수 있다. PC는 단축키나 우클릭 메뉴로, 모바일은 길게 누르기로 한다.
- **지목이 맞으면** 숫자가 원래대로 돌아오고, 공격자가 2초 기절하며, 공격자 게이지가 0이 된다(반사).
- **지목이 틀리면** 지목한 사람이 3초 기절한다.

### 공정성 (가장 중요)
- 판은 **노게스(No-Guess)**로 만든다. 즉 논리만으로 끝까지 풀 수 있어야 한다.
- 거짓말을 주입할 때 솔버가 **"이 거짓말이 이후에 드러나는 정보와 모순을 일으켜 논리적으로 간파할 수 있는가"**를 검증한다. 간파할 수 없는 위치는 공격 대상으로 고를 수 없다.

### 튜닝
- 모든 수치(판 크기, 지뢰 수, 게이지량, 기절 시간, 제한 시간, 동시 거짓말 수)는 **설정 파일 하나로 관리**한다.
- 기본값은 16×16, 지뢰 40개로 시작한다.

---

## 기능 범위

### MVP (1차 출시)
1. **빠른 대전:** 10초 동안 상대를 찾지 못하면 **봇**과 매칭한다(봇 여부는 표시). 봇은 솔버를 기반으로 하며 난이도는 3단계다.
2. **친구 대전:** 방 코드나 링크를 공유해서 1:1 대전을 한다.
3. **튜토리얼:** 30초짜리 봇전이다. 공격 1회와 지목 1회를 강제로 체험하게 한다. 첫 접속 시 자동으로 시작한다.
4. **데일리 퍼즐:** 전 세계가 같은 시드로 혼자 푼다. 결과를 이모지 그리드 공유 카드로 만들어 SNS에 공유할 수 있다. 데일리 순위표도 제공한다.
5. **게스트 계정:** 로그인 없이 익명 ID와 닉네임으로 이용한다.
6. **다국어:** en(기본), ko, ja, zh-CN, es, pt-BR, de, fr로 시작한다.
   - 브라우저 언어를 자동으로 감지하고, 설정 메뉴에서 수동으로 바꿀 수 있다.
   - 번역은 JSON 리소스로 분리하고, 없는 키는 en으로 대체한다.
   - 언어별 URL(`/ko/` 등)과 hreflang을 적용한다.
7. **광고:**
   - 대전이 끝난 뒤 전면 광고를 넣되, **3판에 1번 이하**로 빈도를 제한한다.
   - 메뉴 화면에 배너를 단다. **게임 플레이 중에는 광고를 절대 띄우지 않는다.**
8. **법적·정책 대응:**
   - 개인정보처리방침과 이용약관(다국어), `ads.txt`.
   - **EEA, 영국, 스위스 유저를 위한 Google 인증 CMP 동의 배너.**
   - 쿠키와 로컬 저장소 사용 고지.

### 2차 (MVP 검증 후, 사용자 승인 필요)
- AI 이미지 코스메틱 테마와 보상형 광고 해금, 랭크전(Elo/Glicko), 리플레이 공유, 비대칭 모드, 배틀로얄.

---

## 기술 스택 (권장, 바꾸려면 ADR로 근거를 남기고 사용자 승인을 받을 것)

| 영역 | 선택 |
|---|---|
| 게임 코어 | **Rust 라이브러리 크레이트**: 판 생성, 노게스 솔버, 거짓말 검증, 봇, 규칙 엔진. 결정론적 동작 |
| 클라이언트 코어 | 코어 크레이트를 **WASM**으로 빌드(wasm-bindgen) |
| 프론트엔드 | **TypeScript + Vite + PixiJS**(게임 화면). UI는 **Preact**(React 호환 API, 작은 번들)로 아토믹 디자인 구성 |
| 공유 타입 | Rust 프로토콜 타입에서 TS 타입을 생성(`ts-rs` / `typeshare`) |
| i18n | i18next(또는 동급) + JSON 리소스 |
| 게임 서버 | **Rust (axum + tokio, WebSocket)**. 코어 크레이트를 네이티브로 링크 |
| DB | **PostgreSQL**(Docker) + **sqlx**(컴파일 타임 쿼리 검증) + 마이그레이션(sqlx migrate) |
| 배포 | **Docker Compose**(server, postgres, nginx 정적 서빙, cloudflared) |
| 외부 노출 | **Cloudflare Tunnel** + Cloudflare DNS/CDN/WAF |
| 테스트 | cargo test, proptest, criterion(벤치), Vitest, Playwright, k6, testcontainers |
| 품질 | rustfmt, clippy(`-D warnings`), ESLint, Prettier, 커밋 전 훅 |
| CI | GitHub Actions(무료 한도 안에서): 테스트, 린트, 보안 감사, 빌드 |
| 모니터링 | 헬스체크, 구조화된 로그, Uptime Kuma(선택) |

홈서버 장애(정전, 인터넷 끊김) 동안에는 클라이언트가 "서버 점검 중"을 표시하고, 데일리 퍼즐과 봇전은 WASM 코어로 **오프라인 동작**한다(데일리 기록은 복구된 뒤 제출한다).

---

## 작업 순서

각 단계가 끝날 때마다 사용자에게 짧게 보고하고 확인을 받은 뒤 다음 단계로 넘어간다.

### 0단계: 저장소 전체 초기화 (기존 프로젝트를 전부 밀고 처음부터 시작)
- **사용자가 이미 결정한 사항이다. 기존 프로젝트의 코드, 설정, 테스트, 의존성 파일은 전부 삭제하고 완전히 새로 시작한다.**
- **보존 대상은 이것뿐이다:** `.git/`, `docs/planning/`, `.claude/`(있으면), `.codegraph/`(있으면). 나머지는 모두 삭제한다.
- 실행 전에 삭제 목록을 한 번 보여주고, 사용자가 확인하면 바로 진행한다.
- 새 브랜치(예: `rebuild/liar-sweeper`)를 만들고 그 브랜치에서 삭제부터 시작한다.
- 이 단계에서는 `docs/` 골격과 `.gitignore`만 만든다. 코드 디렉토리 구조는 2단계 설계서에서 확정한 뒤에 만든다.

### 1단계: 기획 문서 — pm-skills 활용 (`docs/product/`)

설치된 pm-skills 플러그인 스킬을 **아래 순서대로 실제로 호출해서** 문서마다 하나씩 작성한다. 앞 문서의 결과를 다음 문서의 입력으로 쓴다. 각 문서 끝에 "결정 사항"과 "열린 질문"을 적고, 열린 질문은 모아서 사용자에게 묻는다.

| # | 스킬 | 산출물 |
|---|---|---|
| 1 | `pm-product-strategy:product-vision` | `01-vision.md` |
| 2 | `pm-market-research:competitor-analysis` | `02-competitors.md`(brainstorm의 경쟁작 확장, 웹 검색 활용) |
| 3 | `pm-market-research:market-segments` + `user-personas` | `03-segments-personas.md` |
| 4 | `pm-market-research:customer-journey-map` | `04-journey.md`(유입 → 튜토리얼 → 대전 → 공유 → 재방문) |
| 5 | `pm-product-strategy:value-proposition` + `pm-marketing-growth:positioning-ideas` | `05-value-positioning.md` |
| 6 | `pm-product-strategy:lean-canvas` | `06-lean-canvas.md` |
| 7 | `pm-product-strategy:monetization-strategy` | `07-monetization.md`(광고 전용 모델, 손익분기: 미니 PC, 전기, 도메인 대비 일 방문자별 수익. 광고 단가는 **추정치임을 명시**) |
| 8 | `pm-product-strategy:swot-analysis` | `08-swot.md` |
| 9 | `pm-marketing-growth:north-star-metric` | `09-north-star.md` |
| 10 | `pm-product-discovery:identify-assumptions-new` + `prioritize-assumptions` | `10-assumptions.md` |
| 11 | `pm-product-discovery:brainstorm-experiments-new` | `11-experiments.md`(가정 검증 실험) |
| 12 | `pm-product-discovery:opportunity-solution-tree` | `12-ost.md` |
| 13 | `pm-go-to-market:gtm-strategy` + `growth-loops` | `13-gtm-growth.md`(광고 없이 시작하는 유입: 데일리 공유, 스트리머, 커뮤니티) |
| 14 | `pm-marketing-growth:product-name` | `14-naming.md`(이름 후보, 도메인 확인은 사용자가 직접) |
| 15 | `pm-execution:pre-mortem` + `strategy-red-team` | `15-pre-mortem.md` |
| 16 | `pm-execution:create-prd` | `16-PRD.md`(위 문서를 종합한 최종 요구사항, 성공 지표 포함) |
| 17 | `pm-execution:user-stories` + `job-stories` | `17-stories.md`(인수 조건 포함) |
| 18 | `pm-execution:prioritization-frameworks` | `18-prioritization.md`(MVP 범위 확정) |
| 19 | `pm-execution:test-scenarios` | `19-test-scenarios.md`(TDD의 출발점) |
| 20 | `pm-product-discovery:metrics-dashboard` | `20-metrics.md`(이벤트 정의, 대시보드) |
| 21 | `pm-execution:outcome-roadmap` + `sprint-plan` | `21-roadmap-sprints.md`(2~3단계 구현 일정) |

스킬 이름이 실제 설치된 이름과 다르면 사용 가능한 스킬 목록에서 가장 가까운 것을 찾아 쓰고, 그 사실을 보고한다.

### 2단계: 상세 설계서와 다이어그램 (`docs/design/`, `docs/adr/`)

모든 다이어그램은 **Mermaid**로 작성해서 GitHub에서 바로 렌더링되게 한다. 문서마다 다이어그램과 설명을 함께 쓴다.

| 문서 | 필수 다이어그램과 내용 |
|---|---|
| `01-architecture.md` | C4 모델: 시스템 컨텍스트, 컨테이너, 컴포넌트(코어, 서버, 클라이언트). 계층(도메인/애플리케이션/인프라/프레젠테이션)과 의존성 방향 |
| `02-deployment-network.md` | 배포 다이어그램: 유저 → Cloudflare(DNS/CDN/WAF) → Tunnel → 미니 PC(Docker 네트워크: nginx, server, postgres, cloudflared). 포트, 내부망 격리, 백업 경로, 장애 시 동작 |
| `03-domain-model.md` | 도메인 클래스 다이어그램(Board, Cell, Match, Player, LieAttack, Accusation, Gauge, Bot 등). 각 클래스의 **단일 책임**을 명시. Rust trait과 struct 매핑 |
| `04-game-rules-spec.md` | 규칙 정식 명세. **상태 다이어그램**: 셀 상태, 플레이어 상태(정상/기절), 매치 생명주기. 엣지 케이스(동시 입력, 끊김과 재접속, 무승부, 공격과 승리가 동시에 일어나는 경우) |
| `05-algorithms.md` | 노게스 생성기, 솔버, 거짓말 간파 가능성 검증, 봇 의사결정. 플로우차트, 복잡도 분석, 벤치마크 목표 |
| `06-protocol.md` | WebSocket 메시지 스키마(버전 포함). **시퀀스 다이어그램**: 빠른 대전 매칭(봇 대체 포함), 친구 방 입장, 대전 진행, 공격과 지목, 재접속, 데일리 제출 |
| `07-database.md` | PostgreSQL **ERD**, 테이블과 인덱스, 마이그레이션 전략, 보존 기간, 백업과 복구(pg_dump 스케줄) |
| `08-client-architecture.md` | 클라이언트 클래스와 모듈 다이어그램(서비스, 상태, 렌더러, WASM 브리지). **아토믹 디자인 컴포넌트 트리**(Atoms~Pages 전체 목록과 재사용 관계). 디자인 토큰 |
| `09-screens-ux.md` | **화면 흐름도**(유저 플로우), 주요 화면 와이어프레임(ASCII 또는 Mermaid), 반응형 기준, 접근성 |
| `10-i18n.md` | 언어 감지와 대체 흐름, 리소스 구조, URL과 hreflang, 번역 워크플로 |
| `11-ads-consent.md` | 광고 노출 시점 상태 다이어그램, 빈도 제한, CMP 동의 흐름 시퀀스, CSP 허용 목록 |
| `12-security.md` | **STRIDE 위협 모델**, 데이터 흐름도와 신뢰 경계, 대응책, 치트 방지 |
| `13-performance.md` | 성능 예산(번들, fps, 지연, 메모리), 측정 방법, 미니 PC 용량 계획 |
| `14-testing-strategy.md` | 테스트 피라미드, 계층별 도구, 커버리지 목표, TDD 작업 흐름, CI 파이프라인 다이어그램 |
| `15-repo-structure.md` | 모노레포 디렉토리 구조, 패키지 경계, 공유 타입 생성 흐름, 빌드 파이프라인 |
| `16-observability-ops.md` | 로그, 지표, 헬스체크, 배포와 롤백 절차, 미니 PC 운영 체크리스트 |
| `docs/adr/` | 주요 결정마다 ADR 1개씩(Rust 코어 공유, PostgreSQL, Cloudflare Tunnel 홈서버, Preact, PixiJS, 서버 권위 모델 등) |

**설계 리뷰 게이트:** 2단계 문서를 모두 작성한 뒤 다음을 셀프 점검한 결과를 `docs/design/00-review-checklist.md`에 남기고, 사용자에게 승인을 요청한다.
- PRD의 모든 요구사항이 설계 문서 어딘가에 대응되는가(추적표).
- 모든 클래스가 단일 책임인가. 중복된 책임은 없는가.
- 모든 테스트 시나리오가 테스트 가능한 경계(인터페이스)에 대응되는가.
- 보안, 성능, i18n, 광고 정책 요구사항이 빠지지 않았는가.
- Mermaid 문법 오류가 없는가.

**사용자가 승인하기 전에는 3단계로 넘어가지 않는다.**

### 3단계 이후: TDD 구현 (설계서의 스프린트 계획을 따름)
각 기능마다 **테스트 시나리오 → 실패하는 테스트 → 구현 → 리팩터링 → 문서 갱신** 순서로 진행한다.
1. 저장소 골격: 모노레포, 린트/포맷/훅, CI, Docker Compose(개발용 PostgreSQL 포함).
2. Rust 코어: 판 생성, 솔버, 규칙 엔진, 거짓말 검증, 봇(proptest와 criterion 포함).
3. 싱글 플레이 웹: WASM 연동, 아토믹 컴포넌트, PixiJS 보드, 봇전, 튜토리얼, 데일리(오프라인), i18n, 설정.
4. 서버와 대전: WebSocket, 매칭, 방 코드, 재접속, PostgreSQL 리포지토리, 데일리 순위표 API.
5. 광고, 법적 대응, SEO: 광고 슬롯, 빈도 제한, CMP, 개인정보처리방침과 이용약관, `ads.txt`, 메타 태그, OG 이미지, hreflang, sitemap.
6. 배포: `deploy/README.md`(미니 PC 초기 설정, Cloudflare Tunnel, 자동 재시작, PostgreSQL 백업과 복구, 업데이트 절차). 국내 통신사 가정용 회선 약관 확인을 사용자에게 상기시킨다.
7. 플레이테스트 준비: 수치 튜닝 설정, 이벤트 로깅(PostgreSQL에 자체 저장. 외부 분석 도구는 사용자 승인 후), k6 부하 테스트 결과 문서화.

설계와 구현이 달라지면 **설계 문서를 먼저 고친 뒤** 코드를 바꾼다.

---

## 작업 규칙

- 사용자와는 **한국어**로 대화한다. 문서는 한국어로 쓰고, 코드 식별자와 주석은 영어로 쓴다.
- 커밋 메시지는 기존 저장소 관례(`Test|...`, `Chore|...`, `Dir|...`, `Docs|...` 등)를 따른다. **커밋과 푸시는 사용자가 요청할 때만** 한다.
- 유료 서비스나 외부 계정이 필요한 작업(도메인 구매, AdSense 가입, Cloudflare 설정)은 **절차만 안내하고 사용자가 직접 한다.**
- 불확실한 수치(광고 단가, 무료 한도, 약관 등)는 공식 문서에서 확인하거나, 추정치라고 표시한다.
- 범위를 넓히고 싶다면(새 모드, 결제, 스팀 등) 구현하지 말고 제안만 한다.
- 문서가 많으므로, 단계별로 진행 상황(완료한 문서와 남은 문서)을 짧게 보고한다.

## 시작할 때 할 일
1. `docs/planning/01-brainstorm.md`와 이 문서를 끝까지 읽는다.
2. 현재 저장소에서 삭제할 대상 목록을 만든다(보존 대상 제외 전부).
3. 삭제 목록을 보여주고, 사용자가 확인하면 새 브랜치에서 전체 삭제를 진행한다.
4. 1단계 pm-skills 기획 문서 작성을 시작한다.
