# ADR0019: 최소 인증 기반과 순차 OAuth 구현

상태: 채택 · 2026-10-08 · FR01/16, NFR01/02 · TS01/20/31~35 · #15/#43

## 배경과 진행 순서

#14는 PR41에서 develop에 병합·종료됐다. OAuth 키 없는 실제 검수만 남기고 개발을 계속하라는 사용자 위임을 따른다. [#43 기반](https://github.com/crystal23733/search-mine/issues/43) → [#44 제공자/HTTP](https://github.com/crystal23733/search-mine/issues/44) → [#45 연결/권리](https://github.com/crystal23733/search-mine/issues/45) → [#46 화면](https://github.com/crystal23733/search-mine/issues/46)을 순차 검사/리뷰/병합/종료한다. [#42 실계정 입력](https://github.com/crystal23733/search-mine/issues/42)은 별도 출시 게이트다. 상위 #15는 실제 검수까지 열어두고 #16은 #46 구현 종료 후 진행한다. 외부 검수를 계약 테스트로 대체했다고 보고하지 않는다.

## 결정

Rust 인증 값 타입·repository/vault 포트와 PostgreSQL/암호 어댑터를 분리한다. 최초 신원 매핑은 nickname 미설정 계정을 만들고 onboarding을 거친다. 비밀번호·메일·사진·전화 컬럼은 없으며 무계정 로컬 연습은 저장소를 호출하지 않는다.

opaque token은 OS CSPRNG 32바이트, URL-safe unpadded encoding이다. DB token/state/browser는 SHA-256 digest만 저장한다. subject는 길이를 구분한 provider/issuer/sub HMAC-SHA256와 key version으로 저장한다. 원문 sub는 요청 메모리만, 키는 외부 주입하고 secret Debug/DTO는 금지한다. 기존 버전 전체 조회·현재 digest advisory lock을 같은 transaction에서 처리한다. 배포 간 current key 혼재를 허용하지 않는다.

거래는 5분, state/provider/browser/intent/account/return path 바인딩이다. return은 Home/Daily/Friends/Settings enum이다. callback은 일치하고 만료되지 않은 DELETE RETURNING으로 한 번 소비한다. 다른 browser/provider 요청은 정상 거래를 소비하지 않는다. 만료 데이터는 cleanup으로 제거하고 이후 HTTP composition에서 주기 실행한다.

PKCE verifier는 교환에 원문이 필요하여 transaction UUID/provider/purpose AAD로 묶은 AEAD 암호문으로 5분 저장한다. nonce는 digest로 검증한다. Apple 철회 credential도 identity UUID/provider/purpose AAD의 AEAD 암호문이며 키는 DB 밖에 둔다. 암호화 nonce는 매번 새 CSPRNG다. 길이/키 버전·tag를 검증하고 변조/다른 AAD/키는 거절한다.

세션은 hash만 저장하고 최대30일이다. 신원 매핑·새 세션 생성·제시된 기존 세션 철회는 원자 처리한다. 서버 시각을 사용하고 미래/만료를 거절한다. 계정 세션 철회/만료 cleanup을 제공하며 쿠키·Origin·CSRF HTTP 연결은 #44다.

닉네임은 NFC,2~16 grapheme, 문자·숫자·조합 마크·ASCII 내부 공백이다. 양끝 공백·control·bidi·HTML·emoji는 거절한다. confusable 이름은 인증 수단이나 전역 유일 키가 아니며 account UUID로 구분한다. 언어별 임의 금칙어 사전은 만들지 않고 운영 정책 검토 대상으로 남긴다.

SQL migration은 별도 도구/계정으로 실행하고 serving 앱은 DML만 사용한다. 동적 sqlx query/bind와 타입 매핑을 실제 PostgreSQL18로 검증한다. query macro를 쓰지 않으므로 offline metadata는 불필요하다. 파괴적 down 대신 추가 migration을 사용한다.

## 제공자 계약과 검증

Google뿐 아니라 Kakao [공식 discovery](https://kauth.kakao.com/.well-known/openid-configuration)/[REST 문서](https://developers.kakao.com/docs/ko/kakaologin/rest-api)가 S256을 명시하여 두 제공자에 강제한다. Apple/Naver에는 확인되지 않은 PKCE를 보내지 않는다. Google discovery는 openid를 지원하지만 OIDC 안내의 scope 설명에는 추가 scope 문구가 있어 openid-only 실제 검수는 #42에 남긴다. email/profile을 임의 추가하지 않는다.

Red→Green으로 Unicode·secret redaction·digest 회전·AEAD 변조/AAD·동시 거래 소비·세션 회전/만료·실DB 제약을 검사한다. 네 제공자/JWKS/HTTP·연결/삭제·UI·실계정은 후속 이슈며 기반 PR을 실제 OAuth 완성으로 보고하지 않는다.
