# #43 최소 계정·인증 거래·세션 기반 검증

2026-10-08 · FR01/16, NFR01/02 · TS01/20/31/32/33/35의 저장·도메인 부분. [ADR0019](../adr/0019-auth-foundation-and-delivery.md)와 [PR47](https://github.com/crystal23733/search-mine/pull/47).

## 구현과 실행 근거

제품 원본 `f51069f3163235ace1834e27cc1b8e7aa0be6f71`에서 Rust AuthService/AuthStore/SubjectDigester/CredentialVault와 PostgreSQL 어댑터를 구현했다. 최초 nickname 미설정 account, 네 provider의 고정 issuer, HMAC subject digest/key version, 5분 거래의 hash/browser/provider/intent/return 바인딩, AEAD verifier/Apple credential 스키마, 30일 opaque session hash를 사용한다. 비밀번호·메일·실명·사진·전화 필드나 자체 비밀번호 API는 없다. secret token/key는 공개 DTO/Debug로 직렬화하지 않는다.

- nickname/token Red2→Green, HMAC/AEAD Red2→Green, start/finish service Red2→Green. 바인딩을 어기는 repository fake의 Red→도메인 재검사 Green도 실행했다.
- [실PostgreSQL Red](https://github.com/crystal23733/search-mine/actions/runs/37652735748/job/112899993579): migration 성공 뒤 미구현 저장소의 Unavailable로 신규 테스트가 실패했다. 기존 readiness는 통과했다.
- [실PostgreSQL 최종 Green](https://github.com/crystal23733/search-mine/actions/runs/37654454229/job/112905903676): 4 passed, ignored0. 별도 migration CLI 실행, 동시 거래 소비1회·동시 identity 생성1계정·기존 키 조회와 현재 digest 갱신·세션 교체/만료·실패 교체 rollback·nickname 저장·provider별 credential FK·token 길이 제약·과잉 컬럼 없음·expiry cleanup·로그아웃/전체 session 철회를 검사했다.
- DML 전용 역할은 login/session DML을 실행하고 CREATE TABLE 및 migration metadata 읽기는 거절했다. 실제 서비스 배포 계정 발급은 #24 운영 절차에서 연결한다.
- 로컬 `scripts/check.ps1` exit0: fmt/clippy/workspace/native/WASM/generated/fixture/token, format/lint/type, Vitest64, production build, Chromium PC/mobile50 passed(58.7s), docs93/Mermaid34 오류0. 로컬 PostgreSQL4개는 ignored이며 통과로 세지 않았다.

암호 zeroize 기능의 lockfile 누락으로 한 중간 CI가 locked 설치에서 실패했다. Cargo.lock에 AES/GHASH/POLYVAL 연결을 포함해 수정했고 locked 검사 조건을 유지했다. 최신 안정 라이브러리는 버전 인자 없는 cargo add로 추가했으며 manifest와 lockfile을 함께 보관한다.

## 커버리지

[실DB 포함 CI 원본](https://github.com/crystal23733/search-mine/actions/runs/37654454229)의 auth-coverage artifact를 받아 `scripts/check_auth_coverage.py`로 다시 계산했다. artifact 보존은7일이다.

| 범위 | 실제 line coverage |
|---|---|
| 인증 도메인 model/service | 242/243 = 99.59% (95% gate 통과) |
| 서버 전체, main/migrate 포함 | 632/692 = 91.33% (80% gate 통과) |
| 기존 web | 1075/1216 = 88.40% |

이는 line coverage이며 전체 branch95/80 달성 주장이 아니다. 로컬에서는 DB를 실행하지 않아 PostgreSQL adapter coverage가41.20%였고 전체80%를 통과했다고 보고하지 않았다. CI는 실제 DB와 ignored test까지 실행해 인증 gate를 계산한다. main/migration entry point를 제외해 비율을 높이지 않는다.

## 코드 리뷰

pm-skills2.1.0 code-review의 정확성·보안 경계 검토를 순차 수행했다. baseline develop31800ef, scope #43 source/SQL/tests/CI. 과잉 claim과 HTTP를 아직 받지 않는 기반이라 HTTP/Cookie/JWKS 안전성은 이 PR의 검토 완료 범위가 아니다.

authority probe: canonical nickname 값만 저장·재조회하고 최초 nickname None을 유지했다. identity probe: 동시 동일 subject 로그인은 한 identity/account이며 provider namespace가 다른 동일 subject는 별도 계정이다. atomicity probe: 세션 hash unique 충돌 시 기존 세션 철회까지 rollback된다. 악성 return/provider·wrong browser/future/expiry·변조 ciphertext·다른 UUID/purpose·closed pool은 거절된다. issuer allowlist와 SQL bind, unique/check/FK/advisory lock이 반례를 막는 근거다. 검사한 범위에서 해결되지 않은 지원 가능한 결함은 찾지 못했으며 전체 서비스가 무결함이라고 선언하지 않는다.

## 후속과 제한

#44에서 실제 제공자 HTTP/JWKS·bootstrap/start/callback/session/logout/nickname과 HttpOnly·Origin/CSRF·주기 cleanup을 연결한다. #45에서 연결·재인증·export/delete/Apple 알림·철회 retry를 구현하고 #46에서 디자인/8언어 UI를 연결한다. 현재 Apple credential은 암호 포트와 DB 제약이며 실제 refresh token 교환/저장/철회는 아직 하지 않았다.

[#42](https://github.com/crystal23733/search-mine/issues/42)는 공개 HTTPS 등록/네 제공자 키/최소 콘솔 권한/실제 계정 검수다. 상위 #15는 이 조건까지 열어둔다. Google openid-only 실제 상호운용과 Apple 실제 알림/철회, 법적 운영자 정책 #21, 홈서버 권한/성능 #24/#26은 미확인이다. 기반 완료를 OAuth 실계정 로그인 완성 또는 법적 준수 증명으로 표현하지 않는다.
