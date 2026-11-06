# ADR0093 · 공개 locale 문서와 검색 메타데이터

- 상태: 채택 · #202(상위 #23) · FR11/13 · NFR01/02/05/06/08 · TS18/19/20/26/30/31
- 승인 근거: 승인된 FR11/설계10, 2026-10-07 순차 전체 위임·외부 입력 이연, 2026-11-06 진행. 제품 범위 추가 없이 독립 구현으로 분해한다.
- 선행: #199/PR200, #198/PR201 MERGED/CLOSED · develop f01bbcd.

## 결정

현재 존재하는 홈·규칙·저장소 고지의 8언어 HTML 24개를 빌드한다. JS 없이 제목·핵심 내용·명시 링크·언어 링크를 읽을 수 있고 실제 앱 시작 직전에 정적 내용만 비워 중복 DOM을 만들지 않는다. 메시지와 Rust 생성 DEFAULT_RULES를 재사용한다. /tutorial은 실제 게임이므로 검색 제외하며 소개/정책 페이지를 가짜로 만들지 않는다. 법적 정책/언어 최종 검수·운영 출구는 상위23/21/147에 남긴다.

VITE_PUBLIC_SITE_ORIGIN은 선택된 HTTPS origin만 받는다. 사용자정보/port/path/query/hash/loopback·IP 호스트를 거절하고 비어 있으면 임의 도메인을 만들지 않는다. 설정 시 공개 문서의 self canonical·8 hreflang+x-default(en)·OG URL·sitemap만 생성한다. 미설정은 URL 메타/sitemap 없이 실행하고 robots.txt는 crawl을 차단한다. 이는 검색엔진 색인 삭제의 보장이 아니다. 실제 도메인/검색엔진 등록과 nginx origin 검수는23/24/147의 외부 출구다.

공개 allowlist 이외의 게임·방·본인·인증·설정·오류 및 query/알 수 없는 hash 문서는 noindex,noarchive이며 canonical/alternate/OG URL을 만들지 않는다. 고정 #main 접근성 앵커는 공개 문서를 유지한다. 요청의 host/query/hash/닉네임·초대·token은 head/정적 내용에 복사하지 않는다. private HTML은 no-store/X-Robots-Tag를 적용하고 API/WS/assets의 응답은 기존대로 유지한다. 루트 fallback index도 검색 제외한다.

하나의 순수 document model로 build/dev/SPA의 제목·설명·URL 분류를 공유하고 preview는 해당 빌드의 HTML/robots/sitemap을 그대로 제공한다. preview 실행 환경이나 현재 소스 메시지로 과거 산출물의 도메인/번역을 바꾸지 않는다. 페이지별 중복 head 효과를 중앙 수명으로 옮기며 route/locale/hash 변경과 unmount에 중복/이전 metadata가 남지 않게 한다. 실제 origin이 있는 build의 런타임 설정도 동일하다. SW/flags/기한/재시도는 바꾸지 않는다.

## 검증과 후속

시나리오→HTTP/DOM Red→Green, 설정 유무/invalid origin, 24 HTML·reciprocal hreflang/x-default·sitemap allowlist, query/HTML 주입 거절·private HEAD, SPA/lang/head 수명, 실제 JS off·PC/mobile keyboard/긴문구를 검사한다. 생성 문서를 기존 PWA precache가 포함하는 실제 build/오프라인 회귀를 확인한다. 실제 서버/도메인·법적 번역/사람 검수와 #83 자연 간헐 실패를 대신하지 않는다. 기존 FR08의 첫 방문 자동 튜토리얼 이동은 유지하며 JS 실행 후 실제 검색엔진의 최종 색인은 상위23에서 검수한다.

공식 근거: [Google localized versions](https://developers.google.com/search/docs/specialty/international/localized-versions), [canonical](https://developers.google.com/search/docs/crawling-indexing/consolidate-duplicate-urls), [Vite plugin API](https://vite.dev/guide/api-plugin).
