# ADR0086 · 동의 HTTP와 실제 DB 작업의 동일 기한

- 상태: 채택 · #186 / 상위21 · 승인된 FR01/13/16·NFR01/02/05/07/08
- 선행: #182·#184/PR185 병합·CLOSED, developfdb7247
- 시나리오: TS20/21/24/29/31/35/36

## 문제와 경계

#184는 선택 저장의 client timeout 뒤 실제 PostgreSQL 대기가 남는 것을 재현하고 transaction-local 기한으로 보완했다. 기존 동의 저장소는 future timeout만 사용하며, HTTP 전체2초와 저장소가 새로 시작하는2초가 다르다. 실제 backend 종료를 같은 기준으로 검증해 보완한다. 외부 키·CMP 선택과 독립적인 기존 서비스의 자원/취소 경계다.

본문 수집 바깥 HTTP middleware가 원래2초의 monotonic deadline을 한 번 만들고 요청 extension으로 전달한다. 동의 port와 전후 세션 확인 port에 동일한 표준 Instant를 넘긴다. budget은 client JSON에 추가하지 않고 내부 호출 문맥으로만 사용한다. 기본 PostgreSQL 직접 호출도2초이며 만료/2초 초과 기한은 DB 작업 전에 unavailable로 거절한다. 본문·인증 확인·pool·verifier IO에 쓴 시간을 경계마다 다시 부여하지 않는다.

ConsentStore는 내부 absolute deadline을 명시한다. PgConsentStore의 직접 호출은 기존 interface/기본2초를 유지하고, HTTP 호출의 문맥은 immutable scoped instance로 적용한다. pool/clock handle만 공유하며 새 pool이나 전역 설정을 만들지 않는다. refresh의 사전 차단 commit와 검증 후 저장 transaction 모두 같은 deadline을 받는다. 검증 IO는 계속 잠금 밖이며 timeout/취소가 이전 grant를 복원하지 않는다.

SessionReader에 deadline-aware 확인을 추가한다. 기존 메모리 port는 읽기 구현을 재사용하며 HTTP의 caller deadline 아래서 동작한다. 실제 PgSessionReader는 같은 deadline을 가진 PostgreSQL transaction으로 session을 조회한다. 기존 일반 세션 조회 SQL/hydration을 공유하며 이 API의 전후 확인에 적용한다. 다른 게임/계정 API의 계약을 바꾸지 않는다.

## 공통 PostgreSQL 기한

#184의 남은 millisecond 계산과 transaction-local transaction_timeout 설정을 crate-private infrastructure helper로 추출해 선택 저장·동의 저장·동의의 세션 확인에서 재사용한다. [ADR0085](0085-atomic-optional-event-storage.md)의 PostgreSQL18 근거와 의미를 유지한다. 남은0/설정 실패는 unavailable이며 전역 DB 설정을 바꾸지 않는다. 실제 backend 종료·미commit rollback·pool 연결 교체와 정상 commit 후 기본 설정 복귀, 더 짧은 기존 DB 기한을 보존한다. COMMIT 응답 불명은 성공으로 위장하지 않으며 기존 CAS/재시도로 확인한다.

account→session 잠금, 현재 token/account/revision·시계/기한, 사전 차단·늦은 CAS·목적별/전체 철회, authority registry와 취소 guard,4동시/4KiB/속도 한도·no-store를 유지한다. 원래 생성 시각·동의 expiry·90일/30일 보존·파일 제거는 바꾸지 않는다. 공개 DTO/생성 TS·migration·의존성을 추가하지 않는다.

## 실제 관측할 결과

실PG의 조회/refresh/목적별·전체 철회에서 요청 timeout/caller취소 뒤 blocker를 해제하지 않고도 원래 backend가 종료돼야 한다. 본문750ms·세션 SQL·pool 대기·검증 IO와 두 transaction이 원래2초에 포함됨을 확인한다. refresh가 잠금 없이 검증하고 사전 차단을 유지하는지·정상 요청/연결 복귀·기존 짧은 기한·deadline 설정 비누출도 검증한다. 원본 SQL 대기 관측→행동 Red→Green 뒤 실제 HTTP/전체 PG·기존95/80·CI/코드 리뷰/정확head를 확인한다.

실제 CMP/수집기·광고/익명 subject·운영147·상위21 전체·83 해결은 이번 완료가 아니다. browser/network flags·검사 timeout/retry를 늘려 통과시키지 않는다.
