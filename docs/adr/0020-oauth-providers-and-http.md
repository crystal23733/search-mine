# ADR0020: 네 제공자와 인증 HTTP 경계

상태: 채택 · 2026-10-08 · FR01, NFR01/02 · TS01/20/31/32/33 · #44

## 선행과 선택

#43는 PR47에서 develop b7d5b36에 병합·종료됐다. #42 실제 키/계정 검수는 별도 출시 조건이다. OAuthProvider/Registry는 authorize/code exchange/verified subject 포트다. HTTP는 AuthService/AuthStore/Vault를 연결하며 브라우저 JWT를 직접 신원으로 받지 않는다.

최신 jsonwebtoken의 표준 verifier로 서명/RS256 allowlist/issuer/audience를 검증한다. 서명 이후 typed whitelist claim에 nonce/azp/exp/iat/nbf를 명시적으로 검사한다. 주입된 서버 시각으로 exp>now, iat는 거래 시작~현재30초 skew, nbf<=now를 적용하며 시간 검사를 생략하는 경로는 없다. nonce는 저장된 digest와 비교한다. 추가 audience는 신뢰하지 않고 자기 client ID만 허용한다. Google의 공식 bare issuer를 URL로만 받는 openidconnect4.0.1은 사용할 수 없고 azp 검증도 자동 제공하지 않는다. JWT 라이브러리를 최신 버전으로 바꾸고 키 검증·claim 정책을 별도 경계로 유지한다. jsonwebtoken은 Apple ES256 client assertion에도 사용한다. reqwest는 TLS·고정 공식 endpoint·redirect none·10초 timeout·bounded response다. 의존성을 구버전으로 pin하지 않는다.

Google/Kakao openid만·S256·state/nonce, Apple scope 생략·code/query·state/nonce, Naver state·서버 POST token/profile id만 사용한다. Google 공식 허용 bare issuer는 서명 검증 후 canonical HTTPS namespace다. 추가 claim은 요청 메모리에서 버리고 DB/DTO/log/event에 쓰지 않는다. Naver token은 JWT가 아니다.

Apple [authorization 최신 경로](https://developer.apple.com/documentation/signinwithapplerestapi/request-an-authorization-to-the-sign-in-with-apple-server.)는 끝 마침표를 포함하고 [client secret](https://developer.apple.com/documentation/accountorganizationaldatasharing/creating-a-client-secret)은 다른 namespace에 있다. HTTPS domain/Services ID, ES256 kid/team/iat/exp/aud/sub, 요청 단위5분 assertion을 사용한다. 검증된 refresh credential은 #45 암호 저장/철회 경계로 넘긴다.

## HTTP와 브라우저

`/api/v1`을 유지하고 GET `/api/v1/auth/bootstrap`을 추가한다. 필수4개 provider 가용 상태·현재 최소 account·메모리 CSRF를 no-store로 반환한다. onboarding 전 nickname은 null, 계정 view는 id/nickname뿐이다. session UUID는 회전 revision으로만 노출하고 token/hash/subject/credential은 공개하지 않는다.

비로그인 bootstrap은5분 `__Host-liar_browser` CSPRNG cookie를 발급하며 계정 생성 없음. Secure/HttpOnly/Path=/, Domain없음, SameSite=Lax다. start/변경은 bounded JSON·exact Origin·browser/session HMAC CSRF를 검사한다. CSRF는5분/메모리만, raw cookie는 JS가 읽지 않는다. callback GET은 Origin 대신 state/browser/provider/expiry/intent를 원자 소비한다. 오류/취소도 바인딩 거래를 소비한다. 성공은 새 `__Host-liar_session` 30일 cookie·같은 보안 속성·세션 회전이며 삭제도 같은 속성이다.

return enum과8locale를 거래에 함께 저장하여 callback 임의 URL/locale 변경을 막는다. nickname 없는 계정은 언어 onboarding으로 돌아간다(#46 UI). PATCH me는 nickname만 검증하며 약관/동의 운영 정책 #21을 완료했다고 가장하지 않는다.

JWKS는128KiB/64키, token/profile은64KiB다. key cache5분, unknown kid 단1회 coalesced bounded refresh. 미설정/실패는 안정 오류·비활성, 성공mock 없음. secret config 오류에 값 미출력. 같은 브라우저 start quota와 callback/query/code 크기·rate를 제한하고 rate key는 장기 DB 저장하지 않는다.

serving DATABASE_URL DML과 별도 migration CLI를 유지하고 만료 cleanup worker를 composition root에 연결한다. 키 없는 기본 실행은 로컬 연습/health를 유지하고 인증은 비활성이다. 테스트 fixture는 별도 composition이며 운영 설정에 인증 우회 API를 넣지 않는다.

## 검증과 후속

authorize 최소 권한·callback, signed fixture/alg/iss/aud/azp/nonce/expiry/iat/JWKS rotation, Naver whitelist, Origin/CSRF/cookie/replay/취소/세션 회전/미설정/timeout를 Red→Green으로 검사한다. HTTP+실DB는 PostgreSQL18 CI다. #45 연결/권리/Apple 암호 저장·알림·철회, #46 화면, #42 네 실계정은 별도다.
