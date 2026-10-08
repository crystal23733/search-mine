# ADR0021 · 명시적 계정 연결·권리와 Apple 철회

2026-10-08 · 채택 · #45 · FR01/16, NFR01/02 · TS20/29/34/35. 선행 #44 PR48은 develop bf375a5에 병합됐고 이슈가 닫혔다. 전체 순차 구현·검토·병합은 사용자 위임 범위다.

## 계정과 세션의 권위

계정당 제공자별 신원은 하나다. 동일 이메일·닉네임 자동 합병은 없다. 연결 시작은 최근 300초 이내 인증된 세션과 CSRF/Origin을 요구한다. 재인증은 기존 연결 제공자의 새 code/state/nonce 검증이며 제공자 비밀번호 입력을 강제하는 비공식 파라미터는 쓰지 않는다. 제공자 SSO가 새 OAuth proof를 발급할 수 있다는 한계를 UI에 설명한다.

Link/Reauth 거래에는 계정 ID와 시작 세션 token hash를 저장한다. callback 현재 cookie와 일치해야 하며 SQL commit 직전에 세션 유효성·계정·최근 인증(연결)·기존 신원(재인증)을 재검사한다. 다른 계정 subject와 같은 계정의 다른 subject는 충돌로 거절한다. 성공 시 이전 세션을 철회하고 새 revision을 발급한다. 마지막 로그인 수단 해제는 거절한다. 해제는 전체 세션을 철회하므로 남은 제공자로 다시 로그인한다.

## 저장소와 권리

AuthStore는 신원 인증 write, AccountStore는 연결 목록·최소 내보내기·삭제·제공자 해제·철회 작업을 맡는다. 로그인 write는 intent/시작 세션과 선택적 Apple credential을 함께 받는다. Pg 어댑터가 결정한 identity UUID를 AAD로 credential을 seal하여 신원·credential·session을 한 transaction으로 commit한다. credential 없는 Apple 로그인을 성공 처리하지 않는다. refresh token은 4096bytes 이하의 요청 메모리와 암호문으로만 취급한다.

내보내기와 삭제/해제는 최근 재인증과 CSRF/Origin을 요구한다. 내보내기는 본인의 내부 ID·nickname·계정 시각·제공자와 연결 시각만 제공한다. digest/subject/credential/session hash/온라인 정답을 반환하지 않는다. 삭제는 계정과 FK cascade 기록·세션·진행 중 거래를 원자 제거하고 복구용 account UUID/삭제 시각 tombstone을 28일 보관한다. 향후 계정 기록 저장소는 동일 account FK cascade/삭제 transaction 계약을 구현해야 한다. 온라인 actor 철회 observer는 #16에서 실제 연결한다. 존재하지 않는 순위/이벤트 삭제 성공을 가장하지 않는다.

## Apple 목적 제한 처리

삭제/해제는 암호화 credential을 identity UUID AAD 그대로 별도 queue로 옮긴다. queue는 계정/subject를 보관하지 않고 24시간 이내 backoff/lease 기반으로 재시도한다. SKIP LOCKED claim과 lease token으로 중복 worker·재시작을 처리한다. 성공/만료 후 삭제한다. provider 장애나 credential 부재는 로컬 개인정보 삭제를 막지 않는다. credential 부재 시 수동 Apple 연결 철회 안내가 필요하다.

정상 계정 credential은 최대 하루 한 번 refresh grant로 확인한다. 고정 Apple token endpoint의 확인된 invalid_grant만 신원 철회의 근거다. 네트워크/서명/JWKS/invalid_client 오류는 계정을 삭제하지 않고 다음 확인으로 넘긴다. refresh ID token 검증은 별도 context이며 로그인 nonce 필수 정책을 약화하지 않는다.

서버 알림은 HTTPS POST `{payload: JWT}`를 받는다. 고정 Apple JWKS·RS256·issuer·명시 설정 audience·jti·iat를 검증한다. 최신 공식 예시의 exp는 선택적이며 있으면 검사한다. iat/event_time은 초 단위, 미래 30초와 과거 24시간을 제한한다(운영 정책). events object와 공식 구형 JSON-string 표현, account-deleted/account-delete 및 consent-revoked를 지원하고 이메일 이벤트/필드는 저장하지 않는다. jti digest receipt 28일과 효과는 같은 SQL transaction에서 중복 제거한다. Apple 제거 시 모든 세션을 철회한다. 다른 수단이 있으면 계정을 보존하고 마지막이면 삭제/tombstone 처리한다. 검증 실패는 개인정보를 변경하지 않는다.

## HTTP와 검증

알림 event_time보다 나중에 연결되거나 새 credential을 받은 신원에는 오래된 철회 효과를 적용하지 않는다. receipt는 그대로 소비하여 동일 알림이 재등록 계정을 지우지 못하게 한다. 이는 서명된 시각·저장된 연결/credential 시각으로 판단하고 request 도착 시각을 철회 시각으로 대체하지 않는다.

`GET /api/v1/me/identities`, `POST /api/v1/me/identities/{provider}/link`, `POST /api/v1/auth/{provider}/reauth`, `DELETE /api/v1/me/identities/{provider}`, `POST /api/v1/me/export`, `DELETE /api/v1/me`, `POST /api/v1/auth/apple/notifications`. 권리 응답도 no-store이며 body allowlist/크기 제한을 적용한다. 익명 401, 재인증 필요 409, 충돌/마지막 수단 409, upstream/DB 실패 503으로 안정된 코드만 반환한다.

TDD로 세션 변경·재인증 경계·subject 충돌·마지막 수단·원자 삭제/credential 저장·서명 변조·알림 중복·lease/expiry·장애 시 로컬 삭제를 검증한다. 실제 PostgreSQL CI에서 동시 요청/롤백/FK cascade를 검사한다. 실제 Apple 알림 audience 등록·실계정 철회·Google 최소 scope 호환성은 #42 외부 검수이며 fixture 통과로 대체하지 않는다.

공식 근거: [Apple 계정 변경 알림](https://developer.apple.com/documentation/signinwithapple/processing-changes-for-sign-in-with-apple-accounts), [TN3194](https://developer.apple.com/documentation/technotes/tn3194-handling-account-deletions-and-revoking-tokens-for-sign-in-with-apple), [구형 서버 알림 설명](https://developer.apple.com/videos/play/wwdc2022/10122/), [Google OIDC reference](https://developers.google.com/identity/openid-connect/reference). 실제 console의 notifications audience는 별도 명시 설정하고 임의 추정하지 않는다.
