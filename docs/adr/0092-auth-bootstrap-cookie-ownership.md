# ADR0092 · 세션 조회와 인증 쿠키 쓰기의 소유권

- 상태: 채택 · #198 · FR01/16·NFR01/02/05/08 · TS20/29/34/35
- 선행: #199/PR200 MERGED/CLOSED · developc611ad5

## 관측과 재현 범위

PR197의 원격 trace는 재인증 시작 후 중단된 bootstrap과 새 문서의 account/session 없음·삭제 버튼 이탈을 보여 준다. callback의303/Set-Cookie 전체와 중단된 응답 헤더가 모두 보존되지는 않아 원격 실패의 유일 원인으로 단정하지 않는다. 실제 HTTP bootstrap은 요청의 세션이 현재 무효하면 세션 cookie 삭제를 응답한다. 요청이 캡처한 이전 세션과 응답 시점 브라우저의 최신 세션이 다를 수 있으므로, 과거 조회가 재인증으로 교체된 쿠키를 삭제하는 순서를 통제해 재현한다.

[RFC6265 §4.1.1](https://www.rfc-editor.org/rfc/rfc6265#section-4.1.1)은 동시 Set-Cookie 응답의 경합을 설명한다. 여기서는 세션 조회 결과와 인증 쿠키 변경의 소유권을 분리한다. callback/명시 logout/계정삭제 등 인증 변경만 세션 cookie를 쓰며 bootstrap은 무효 세션에서도 세션 cookie를 삭제/재발급/갱신하지 않는다. 브라우저 CSRF 바인딩 cookie의 필수 발급은 유지한다.

## 보안 계약

형식이 잘못된/중복 Cookie 거절은 기존대로 유지한다. 형식이 유효하지만 서버에서 무효/만료/철회된 세션은 계속 account=null/session_revision=null이고 보호 API는401/403으로 거절한다. 쿠키를 남긴다는 것이 세션 권한을 연장하거나 복원하는 의미는 아니다. 새 세션 발급은 검증된 OAuth callback, 최근 재인증/권한 변경은 기존 서비스/저장소의 검증을 그대로 따른다. 30일 cookie 만료는 읽기로 연장하지 않고 명시 logout/삭제의 만료 cookie도 유지한다.

bootstrap CSRF는 실제 요청의 browser/session cookie에 바인딩한다. 무효 세션을 메모리에서 제거해 다른 CSRF를 만들지 않는다. 이로써 무효 cookie가 남은 guest도 정확한 Origin·browser/session HMAC을 사용해 명시 login을 시작할 수 있다. 다른 cookie/과거 세션의 CSRF를 새 세션에 재사용하거나 무효 cookie로 link/reauth/본인 조회를 허용하지 않는다. 계정 생성은 OAuth 검증 뒤에만 한다.

## 검증

제품 코드 전에 시나리오를 기록하고 실제 HTTPS/PG에서 이전cookie캡처→재인증/cookie회전→지연된 이전cookie조회응답→새cookie보존·새문서본인화면·새로운명시삭제확인을 Red/Green으로 검사한다. APIRequestContext의 실제 browser cookie jar를 사용하고 헤더를 가짜 성공으로 덮어쓰지 않는다. native HTTP에서는 무효cookie의guest/최소DTO·세션Set-Cookie없음·정확CSRF로새login가능/불일치거절·보호API거절을 검증한다.

원래 재인증 복귀 E2E는 기존 URL/이름이 남아 있는 문서를 성공으로 보지 않도록 새 문서의 performance.timeOrigin을 관측한 뒤 현재 화면/명시 확인을 검사한다. 고정 sleep·timeout/retry/권한 완화는 하지 않는다. 원격 trace와 통제된 경합 재현을 구분하고 실DB/HTTPS·도메인/전체coverage·전체회귀·문서/Mermaid·정확head 리뷰/CI/병합으로 마무리한다. 실제 제공자42와 오프라인83의 완료를 대신하지 않는다.
