# 공개 문서 SEO 설정

#202/ADR0093은 홈·규칙·저장소 고지 3종×8언어의 정적 HTML을 제공한다. 본인/계정·로그인·설정·게임·오류 문서는 검색 제외하며 이 설정은 API의 인증 권한을 대체하지 않는다. 실제 정책·튜토리얼 소개·번역의 사람 검수와 최종 검색 검수는 #23/#147에 남아 있다.

## 도메인이 없을 때

`VITE_PUBLIC_SITE_ORIGIN`을 비워 둔다. 공개 HTML·기존 앱은 동작하지만 canonical/hreflang/OG URL·sitemap은 생성하지 않는다. `/robots.txt`는 `Disallow: /`, `/sitemap.xml`은404다. 요청 Host나 시험 도메인을 기본값으로 사용하지 않는다. robots 차단만으로 기존 검색 색인의 삭제를 보장하지 않는다.

## 확정된 도메인을 연결할 때

운영자가 선택한 실제 HTTPS origin을 `VITE_PUBLIC_SITE_ORIGIN`으로 빌드 환경에 전달한다. HTTPS·ASCII 호스트의 정규 origin이며 끝의 `/`만 선택적으로 허용한다. 사용자정보·명시 port·다른 path·query/hash·IP literal/localhost는 거절한다. 설정값은 비밀 키가 아니며 실제 DNS/사이트 소유권 검증은 운영자가 수행한다.

- 빌드 시 이 값으로 canonical·각 문서의8개 hreflang와 en x-default·OG URL·24개 URL의 sitemap을 생성한다.
- 클라이언트 번들에도 같은 값을 고정한다. 변경하면 다시 빌드한다. dev/preview는 같은 환경값으로 실행해 HTML과 SPA가 일치하도록 한다.
- 실제 문서 URL은 `/{locale}/`, `/{locale}/rules`, `/{locale}/storage`다. 산출 파일은 `{locale}/index.html`, `{locale}/rules.html`, `{locale}/storage.html`이다. Vite dev/preview는 동일 모델로 응답한다. #24의 nginx는 이 깨끗한 문서 URL을 해당 정적 파일에 연결하고, 개인/게임 fallback·오류 HTTP 상태/검색 제외·no-store를 별도로 검수해야 한다.
- query 또는 알 수 없는 fragment가 있는 공개 화면에도 URL 메타를 넣지 않고 검색 제외한다. 고정 접근성 `#main`만 허용한다. 요청 값/닉네임/초대/토큰을 head에 복사하지 않는다.

Google은 공개 문서의 상호 언어 링크와 self canonical을 읽는다. sitemap에는 공개 allowlist만 포함한다. [공식 다국어 문서](https://developers.google.com/search/docs/specialty/international/localized-versions), [canonical 문서](https://developers.google.com/search/docs/crawling-indexing/consolidate-duplicate-urls).

## 출시 전 검수

실제 도메인 HTTPS/redirect/nginx 응답과 8언어 공개 내용, 상호 링크/사이트맵, 개인정보/게임의 noindex·no-store를 확인한다. 기존 FR08의 첫 방문 자동 튜토리얼 이동은 유지하므로 실제 검색엔진이 JavaScript까지 실행한 뒤 보는 최종 문서/색인은 #23에서 별도로 검수한다. Search Console 등록이나 색인 성공은 로컬 HTML/브라우저 테스트로 완료 판정하지 않는다. 정책 운영자 정보·법적 번역·CMP/광고 계정 설정과 #83의 자연 간헐 문제도 별도다.
