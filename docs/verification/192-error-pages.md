# #192 · 8언어403/404와 문서 오류 응답

[ADR0089](../adr/0089-error-document-pages.md) · FR10/11·NFR01/02/05/06/07/08·TS18/19/20/26/30/31.

선행190/PR191 MERGED/CLOSED·최신develop4ae2f76 clean/origin0:0 확인. 설계/시나리오를 제품 코드보다 먼저 기록했다. 실제 Red/Green·검증/리뷰·PR/CI/병합은 수행 후 기록한다. 실제운영origin/admin·외부147/42·상위21/83 전체완료가 아니다.

## 실제 구현과 검증

공통 document-route 카탈로그를 App과 Vite middleware에서 공유한다. /403·/404·unknown의8언어 안내는 고정locale href로 query/hash를 제거하며, 제목/noindex·키보드 포커스를 진입/이탈 수명에 적용한다. API/WS/health/assets와 dev 소스는 HTTP 오류 문서 처리에서 제외한다. middleware는 고정된 entry만 읽고 HTML GET/HEAD에 실제4xx/no-store/nosniff/no-referrer/X-Robots-Tag를 반환한다. SPA·캐시된 offline 이동과 새 origin 응답을 구분한다.

- 실제 Red: 기존 UI의404/403 제목 없음2개(기존5PASS), 직접 기동한 preview4187의 unknown HTML GET200대404 실패1개를 실행했다. 자체 preview는 종료했다.
- 구현 뒤 기존 unknown 안내 테스트를 새404 계약으로 갱신했다. test matcher 누락/포커스 hook 배치 오류는 행동 Red로 계산하지 않았고 수정했다. 비동기 head cleanup의 이전 title 복구 문제를 발견해 layout 단계에서 관리하도록 보완했다. 오류→정상/개인화면→오류→정상의 robots/title 수명·첫방문/고정href/미반영을 포함한 집중 App8개 통과.
- 전체 웹402개 통과. 로컬 라인 커버리지는91.23%(3704/4060)이다. lint/typecheck/format·production build 통과. 새 config import 경고는 explicit .ts와 noEmit tsconfig의 allowImportingTsExtensions로 해결했고 Vite native loader build도 통과했다. 경고 억제/구버전 pin/의존성 추가는 없다.
- 실제 browser/HTTP4개 통과(3.6초): dev/preview×8locale×403/404×PC/mobile360의64화면·origin403/404/no-store/robots/nosniff·HTML/query값미반영·HEAD본문0·정상문서/asset응답보존·비HTML요청·키보드홈복귀/query/hash제거·정상화면검색제외복구/가로넘침0을 확인했다. 실제 APIRequestContext로 origin을 별도 관측해 SW/SPA응답과 혼동하지 않았다. ko403/404의 PC/mobile4개 screenshot을 .tmp/error-layout에 보존하고 직접 시각 검토했다.
- Rust/API/DB/게임/SW source는 바꾸지 않았고 이번 로컬 Rust/실PG 전체는 재실행하지 않았다. 로컬 PowerShell전체/LLVM·전체브라우저는 미실행이며 PR의 필수6checks/전체회귀·coverage·최신head를 확인한다. 문서237개/Mermaid35개 통과.

## 코드 리뷰와 범위

사용자 path는 고정 entry read의 파일 경로가 되지 않는다. URL/query/hash를 HTML/화면 문구에 반영하지 않고 링크는 locale 고정 href로 원래query/hash를 지운다. 공통문서분류와 독립된 backend 권한/API/WS·소스/asset 경계를 유지하며 개인정보 화면과 오류 head 수명이 서로 남지 않는지 검증했다. HTTP4xx middleware는 HTML GET/HEAD에만 적용하고 기존 handler의 응답을 변환하지 않는다. PWA/기존browser flags·검사timeout/retry/권한·secret·다운로드를 바꾸지 않는다.

실제운영origin/nginx의 같은계약 적용/실호스트 검수는24, 내부admin은 별도 설계/구현이다. 외부147/42·상위21/83 전체완료로 보고하지 않는다. PR CI/정확head·최종리뷰/병합은 GitHub PR과 로컬 당일 review에 기록한다.
