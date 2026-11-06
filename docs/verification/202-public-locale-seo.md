# #202 · 공개 locale HTML/SEO

[ADR0093](../adr/0093-public-locale-documents.md) · FR11/13·NFR01/02/05/06/08·TS18/19/20/26/30/31.

선행199/198과 PR200/201 병합·CLOSED, 최신 developf01bbcd clean을 확인했다. 사용자 위임에 따라 외부 입력 없는 공개 문서 구현을 상위23에서 분리한다. 설계/시나리오를 제품 코드보다 먼저 기록했다. 실제 Red/Green/코드 리뷰·검증/CI/병합은 실행 후 기록한다. 상위23/21/22/147·실제 도메인/정책/언어 최종 검수와83은 별도 미완료다.


## 실제 구현과 검증

- 제품 코드 전 시나리오/ADR 기록 후 실제 browser Red1을 확인했다. JS-off `/en/`이 lang=ko/빈 내용으로 실패한 raw/trace는 `.tmp/seo202-browser-red.log`/`seo202-browser-red-artifacts`에 보존했다.
- 순수 document 모델/중앙 head 소유권과 Vite build/dev/preview adapter를 연결했다. 3공개종×8언어 HTML, 설정 시 self canonical/8 reciprocal hreflang+x-default/en·OG URL/sitemap24를 생성한다. 기본 미설정은 URL 메타0·robots Disallow/사이트맵404다.
- HTML/XML escaping·알 수 없는 경로/query/hash·개인/게임 noindex·요청 Host 미사용·no-store/HEAD·API/assets pass-through를 검사했다. main은 앱 시작 직전에 정적 root를 비워 중복 heading/DOM을 남기지 않는다.
- 초기에 JSON import attributes를 브라우저 dynamic loader에 적용한 시도는 dev locale 회귀를 만들었다. 실제 실패 trace를 `seo202-dev-locale-regression-artifacts`에 보존하고 공통 i18next engine/브라우저 dynamic import/빌드 고정 파일 reader를 분리했다. 단순 warning 억제나 flags 우회 없이 최종 dev/production 검사에서 복구를 확인했다.
- 전체 웹436 PASS, 라인3870/4231=91.46%. 새 document/head·공통 translation engine/rule-copy의 라인은 각100%다. 관련4파일32개 및 최종 build 계약13개 통과. 이 수치는 로컬 실제 측정이며 원격 CI와 구분한다.
- 최종 PC1440/모바일360의 JS-off8언어24문서×dev/preview, 키보드 skip→main focus·German 긴문구/가로넘침, SPA locale/hash/private head와 기존403/404 검사8개가5.1초에 통과했다. 화면 여백은 기존 app-shell/skip-link CSS를 재사용했다. 기계 검증과 화면 관찰을 전문 번역/실제 사람 검수로 부르지 않는다.
- 별도의 깨끗한 시험 origin production build를 실제 모바일 브라우저에서 검사했다. JS-off24문서의 canonical/9 alternates/OG, sitemap24/robots, JS-on locale→내정보→홈→private fragment→#main의 head 수명/중복0이 통과했다. 시험 도메인/마커가 기본 production dist에 없음을 확인했다.
- lint/typecheck/format/build·문서248/제품21/설계18/추적36·Mermaid34·diff 검사 통과. 기본 build PWA precache는96개이며 worker 소스/flags/retry/기한은 바꾸지 않았다. 로컬 전체 auth/DB/LLVM·전체 browser는 미실행이며 필수 원격 전체 CI로 검증한다.

## 실제 코드 리뷰와 남은 출구

head는 소유한 data-liar-seo 노드만 정리하고 private 문서로 이동할 때 URL 메타를 제거하며 unmount에 원래 노드/제목/언어를 복원한다. locale 로딩 중 잘못된 언어의 canonical을 만들지 않는다. build는 모든 메시지 준비 Promise를 공유해 동시 첫 요청의 부분 번역을 막고 요청값으로 파일을 고르지 않는다. 공유 rule-copy는 Rust 생성 DEFAULT_RULES의 UI 포맷만 다루고 게임 판정을 복제하지 않는다. 계정/게임 DTO·인증·광고/동의·의존성/lockfile은 변경하지 않았다.

실제 도메인/DNS/nginx·Search Console/색인 성공, 법적 정책·튜토리얼 소개·전체 번역 사람 검수는 상위23/21/147/24에 남긴다. FR08 첫 방문 자동 튜토리얼 이동도 유지하므로 JS 실행 후 검색엔진의 최종 문서는 별도 검수한다. 이 하위 구현으로 상위23/22를 종료하거나83의 자연 간헐 원인을 해결했다고 보고하지 않는다. CI/정확 head 최종 리뷰/병합은 PR과 로컬 당일 review에 기록한다.
