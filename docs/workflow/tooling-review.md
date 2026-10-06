# #4 개발 환경 구현 경계

TS30의 Rust public DTO→생성 TS 검증 경로, Rust/TS 단위 검사, 실제 PostgreSQL readiness 통합 검사, Vite build와 Playwright 실행 경로를 준비한다. core/wasm/server/protocol의 workspace 경계를 만들고 게임 규칙/봇/실OAuth 기능은 후속 이슈에서 구현한다.

첫 의미 있는 계약은 공개 account projection에 내부 id·게임용 nickname만 포함하고 공급자 신원/비밀번호/이메일을 포함하지 않는 것, liveness가 DB와 무관하게 동작하고 readiness는 실제 DB 연결 가능 여부를 반영하는 것이다. 테스트를 먼저 실행해 실패를 확인한 후 구현한다.

Preact 화면은 최소 빌드 진입점만 준비한다. 제공된 디자인의 토큰·18개 화면은 #10 이후 구현하고 아직 완성된 게임/로그인을 표시하지 않는다. 라이브러리는 추가 명령에 버전을 지정하지 않고 최신 안정 버전을 받으며 lockfile을 검토한다. 초기 개발 DB는 loopback에만 노출하고 운영 DB의 host 포트 금지 규칙과 구분한다.

## 실행 근거

- Red: 공개 account JSON에 nickname 누락, `/health/live` 404≠200, `/health/ready` 404≠503, DOM에 product heading 없음. 컴파일/러너 실패를 행동 실패로 계산하지 않았다.
- Green: public account id/nickname만, liveness no-store, DB 미설정 readiness503. Rust 단위3개·웹 단위1개·Chromium desktop/mobile E2E2개 통과.
- fmt/clippy/native/WASM 컴파일·Rust TS 생성 일치·Mermaid34 실제 parse 확인.
- 최신 TypeScript7과 typescript-eslint의 지원 범위 충돌 → [TypeScript7 기반 Oxlint](https://oxc.rs/docs/guide/usage/linter)로 변경. 최신 Vitest5/jest-dom 타입 충돌 → 기본 assertion과 실제 Playwright visibility 검사로 분리; 숨기는 skipLibCheck는 쓰지 않았다.
- Docker Desktop의 engine이 로컬에서 ready가 되지 않았다. 실제 PostgreSQL 검사는 database CI에서 실행하고 결과를 PR에서 확인한다. 로컬 미실행을 통과로 보고하지 않는다.
- 코드 리뷰: 도메인 crate infra 의존 없음, 공개 프로토콜 secret 필드 없음, generation check 읽기 전용, HTTP health 내부 오류 미노출·timeout/no-store, 기본 bind loopback, 개발 DB port loopback만, CI read 권한.

현재 Web gzip JS 약5KiB는 브랜드 진입점만의 크기다. Pixi/WASM/게임을 포함한 최종 번들 예산 달성 근거가 아니다. core/wasm 게임 동작과 OAuth 실인증은 아직 구현되지 않았다.
