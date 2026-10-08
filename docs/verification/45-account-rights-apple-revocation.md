# #45 계정 권리·Apple 철회 검증

FR01/16, NFR01/02 · TS20/29/34/35 · M3 · 선행 #44 PR48 병합/종료 · [PR49](https://github.com/crystal23733/search-mine/pull/49). 계약은 [ADR0021](../adr/0021-account-rights-and-apple-revocation.md), [HTTP](../design/06-protocol.md), [DB](../design/07-database.md), [운영 설정](../workflow/oauth-configuration.md)을 따른다. 수치는 [auth-rights-summary.json](auth-rights-summary.json)에 기록한다.

## 구현한 동작

연결/재인증 거래를 시작 계정·세션 hash·provider·locale·허용 목적지에 묶는다. 연결·해제·내보내기·삭제는 서버가 검증한 최근300초 인증을 요구한다. 다른 계정 subject 연결과 마지막 제공자 해제는 거절하며, 해제/삭제는 해당 계정의 전체 세션과 거래를 철회한다. 계정당 provider 하나의 SQL 제약과 계정 행 잠금으로 동시 해제에서도 로그인 수단을 보존한다. 닉네임 미설정 계정도 자기 개인정보 권리를 행사할 수 있다.

내보내기는 내부 UUID·nickname·생성/최근 접속 시각·provider/연결 시각뿐이다. 원문 subject·digest·cookie/token/hash·Apple credential을 반환하지 않는다. 익명 bootstrap/로컬 연습은 계정을 만들지 않는다. 브라우저 mutation은 exact Origin·메모리 CSRF·strict4096byte JSON·no-store를 적용한다. Rust AuthIdentity/Export/Erasure에서 TypeScript 선언을 생성한다.

Apple refresh는 PostgreSQL 어댑터가 결정한 identity UUID와 목적을 AAD로 AEAD 암호화하여 identity/session과 한 transaction에 저장한다. seal/unique 실패는 기존 세션과 신원을 보존한다. 삭제는 계정 FK cascade와28일 UUID tombstone, 철회 목적의 최대24시간 암호화 queue를 원자 commit하며 외부 장애나 credential 부재로 로컬 삭제를 막지 않는다. 직접 연결 철회가 필요하면 응답에 수동 안내 flag를 반환한다.

worker는 고정 Apple endpoint·짧은 ES256 assertion·10초 철회/20초 확인 deadline과60초 tick을 사용한다. SKIP LOCKED claim·60초 lease/UUID·backoff로 중복 요청/재시작/오래된 완료를 처리한다. 정상 credential은 하루 한 번 확인하며 정확한 refresh grant의 HTTP400 invalid_grant만 철회 근거다. outage/invalid_client/서명/JWKS 오류는 계정을 삭제하지 않는다. 오래된 일일 결과는 ciphertext와 check revision이 일치할 때만 적용한다.

서버 알림은 고정 RS256/JWKS·issuer·명시 audience·iat/event_time·선택 exp·jti를 검증한다. 최신 object와 구형 JSON-string events, account-deleted/account-delete·consent-revoked를 지원하며 이메일 값은 저장하지 않는다. receipt digest와 삭제 효과를 원자 처리하고, 알림 시각 이후 갱신된 신원/credential은 오래된 알림으로 철회하지 않는다. 다른 제공자가 있으면 계정을 보존한다. audience 미설정은503이며 실제 console 값을 추정하지 않는다.

## 실제 실행과 TDD

계정 start의 무권한 수락, 실DB Apple login/erase/queue claim/알림/일일 처리, 권리 route404, worker Unavailable와 잘못된 audience 설정을 실제 Red로 실행한 뒤 Green을 확인했다. 컴파일 오류를 행동 Red로 계산하지 않았다. 중간 a4cfcfa 전체 서버 coverage79.68%는 실패였으며 의미 있는 HTTP/동시성 검증을 추가한 뒤 목표를 충족했다.

제품 소스 **b9dd852100de91437d4499896e57a6e2fdf63ef8**의 [PostgreSQL18 CI](https://github.com/crystal23733/search-mine/actions/runs/37769639633/job/113285844944)에서 별도 migration CLI 및 **통합16개 passed/ignored0**를 확인했다. DML/DDL 분리, 거래 소비/세션 회전, 전체 권리 HTTP, foreign subject/최근 인증 경계, Vault 실패 rollback,2동시 해제/삭제와 FK 기록 제거, signed notification HTTP/중복·지연, daily check revision, worker outage/재시도, queue 동시 claim/lease/24시간 만료를 포함한다. 각 테스트는 UUID schema로 격리하고 제거한다. 로컬 Docker daemon은 사용 불가였고 해당16개는 로컬 ignored로 남겼다.

같은 CI의 auth-coverage 원본 JSON을 다운로드하여 check_auth_coverage.py로 재계산했다. **model/service369/371=99.46%, 전체 서버2750/3057=89.96% lines**. tests/examples는 제외하고 main/migrate coverage0도 전체에 포함했다. inline 테스트를 제품 소스에서 별도 tests 모듈로 옮겼으며 branch coverage/성능/실계정 성공을 주장하지 않는다.

dc6a741 제품 상태에서 scripts/check.ps1 exit0: workspace fmt/clippy/native/WASM·생성 타입·fixture/tokens, 웹 format/lint/type/Vitest64(88.40% lines)/build, PC/mobile/offline Chromium50, docs98/Mermaid34. 이후 경합 수정 소스의 local server42개와 clippy를 다시 실행했다. 수정 이후 전체 검증은 GitHub 최신 head의 필수6checks에서 확인한다.

## 코드 리뷰와 수정

pm-ai-shipping code-review2.1.0, correctness/security, scope45/baseline develop bf375a5 기준으로 순차 검토했다. Cookie/Origin/CSRF→현재 세션→transaction→provider proof→account/identity 잠금→credential/session commit→최소 DTO, 알림 signature/time/jti→identity generation→transaction 효과, encrypted queue→lease→고정 revoke, 일일 refresh→서명/subject digest→check revision 경계를 양쪽에서 읽었다.

authority는 요청 account/subject가 서버 세션과 다른 충돌과 미정의 body 거절로, correlation은2동시 unlink/delete·오래된 lease/ciphertext/revision·지연 알림으로 강제 검사했다. 추가 로그인/해제 경합을 발견하고 수정했다. find_identity 이후 account lock 대기 중 제공자가 해제되면 과거 조회로 session을 발급했다. subject advisory lock은 unlink와 공유하지 않아 반증하지 못했다. backend PID와 pg_blocking_pids로 순서를 확정한 [실DB Red](https://github.com/crystal23733/search-mine/actions/runs/37769310228/job/113284804933)는15통과/1실패와 실제 세션 발급을 확인했다. 계정 lock 후 identity UUID/owner를 다시 조회하여 제거/교체된 신원을 Unauthenticated로 거절한 뒤16개 Green이다.

수정 후 검토 범위에서 지원되는 미해결 정확성·보안 결함을 발견하지 않았다. 모든 코드/운영 환경에 대한 무결함 증명은 아니다.

## 미확인 범위

실제 제공자 키/console 최소 권한·Google openid-only 호환성·Apple 알림 audience/철회는 #42, 로그인/계정 화면은 #46, 온라인 actor 철회 연동은 #16, 실제 백업 복구/tombstone 적용은 #25에서 검증한다. queue의24시간 claim 금지와 만료 row 제거는 구분하며 물리 제거는60초 cleanup/복구 운영 절차에 의존한다. 처리량/분산 공격/실기기/미니 PC 성능/법적 출시 고지는 이번 검증 범위가 아니다.
