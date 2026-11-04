# ADR0089 · 8언어403/404와 문서 오류 응답

- 상태: 채택 · #192 · FR10/11·NFR01/02/05/06/07/08
- 선행: #190/PR191 병합·CLOSED / develop4ae2f76
- 시나리오: TS18/19/20/26/30/31

## 사용자 화면

사용자의 남은 독립 작업 진행 지시에 따라 기존 공통 준비 안내를 명시적403/404로 보완한다. /{locale}/403은 접근권한 안내, /{locale}/404와 미지원 문서 주소는 찾을 수 없음 안내를 제공한다.8언어·공통Card/버튼토큰·키보드/긴문구·PC/mobile에서 검증한다. 권한 검사는 기존 서버 API에서 계속 수행하고 안내 페이지에 계정/운영자 권한을 부여하지 않는다.

code/title/설명과 현재locale의 고정 홈 링크를 제공하며403에는 계정 링크도 제공한다. 요청URL·query/hash·식별자를 화면에 반영하지 않고 링크는 plain href로 원래query/hash를 제거한다. 문서title은 해당code/번역제목이고robots noindex/noarchive를 설정하며 이탈 시 이전 head 상태를 복구한다. 첫방문튜토리얼이 오류안내를 대체하지 않는다. 모르는locale은 기존en fallback을 따른다.

## 문서 경로와 HTTP

App과 dev/preview가 작은 공통 웹문서 경로 카탈로그를 사용한다. 이는 화면URL분류이며 게임/동의/권한 규칙·Rust API DTO를 복제하지 않는다. 기존 알려진 게임/계정/설정 경로는 정상 분류하고 unknown 및 명시 오류경로를4xx로 분류한다.

Vite middleware는 GET/HEAD의 text/html navigation만 처리하고 API/WS/health/assets 등 비문서는 기존handler에 넘긴다. 오류 문서일 때만 검증된 고정 root/index.html(dev) 또는 build outDir/index.html(preview)을 읽는다. 요청path를파일경로에붙이지않고 dev는 transformIndexHtml로 정상 entry/HMR을 주입한다. 실제403/404와 text/html·no-store·nosniff·no-referrer·X-Robots-Tag noindex/noarchive를 직접 반환해 기존SPA fallback의200에 의존하지 않는다. HEAD는 body를 반환하지 않는다. 알려진 정상 문서/자산·API 응답/WS·권한 처리를 바꾸지 않는다.

SPA history 이동과 캐시된 offline 문서는 클라이언트의 오류안내/검색제외를 보장하지만 네트워크 응답이 새로 발생한 것으로 보고하지 않는다. SW와 기존 진단/timeout/retry/브라우저 flags는 수정하지 않는다. 운영 nginx/origin #24에서도 같은 직접 문서4xx·API/WS pass-through·HTML no-store/robots 계약을 적용하고 실호스트 검수는 그 이슈에 남긴다. dev/preview 성공을 운영 배포 완료로 표현하지 않는다.

## 검증

설계/시나리오 뒤 기존unknown 안내·preview200의 행동 Red를 실제 확인한다. 화면의명확한code/title·오염URL미반영·안전한고정복귀/locale·head복구를 단위/실브라우저로 검사한다. 직접GET/HEAD·HTML오류4xx/헤더·정상문서/asset/API guard를 실제previewHTTP로 확인하고8locale×PC/mobile·키보드/가로넘침·오프라인 UI/기존전체회귀·문서/Mermaid·6checks/정확head·코드리뷰 뒤 병합한다. 실제admin/운영24·외부147/42·상위21/83 전체는 별도다.
