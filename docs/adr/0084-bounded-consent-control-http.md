# ADR0084 · 현재 인증 세션 동의 HTTP 경계

- 상태: 채택 · #182 / 상위 #21 · 기존 승인 FR01/13/16·NFR01/02/05/07/08
- 선행: #20·#174·#178/PR181 병합·CLOSED, develop90a55b9
- 시나리오: TS20/21/24/29/31/35/36. 외부 CMP/광고 계정과 독립적인 서버 연결이다.

## 현재 세션과 닫힌 입력

POST `/api/v1/me/consent`는 `{v:1}`로 현재 상태와 서버 revision을 조회한다. POST `/api/v1/me/consent/refresh`는 `{v:1,expected_revision:null|UUID}`로 서버 ConsentVerifier의 재검증을 요청한다. POST `/api/v1/me/consent/withdraw`는 `{v:1,purpose:"analytics"|"advertising"|"all"}`로 목적별 또는 전체 철회를 원자 저장한다. UUID는 canonical non-nil이고 입력은4KiB·고정 경로/JSON MIME·query 없음·알려진 필드만 허용한다.

계정·세션·subject는 현재 opaque cookie와 서버 SessionReader로 도출한다. client account/subject/revision 발급값·grant boolean·TC 문자열은 받지 않는다. expected_revision은 서버가 발급한 현재 revision의 CAS 조건일 뿐이며 권한 증거가 아니다. Origin/CSRF가 필요하고 닉네임·최근300초 재인증은 필요하지 않다. 무계정 로컬 연습의 계정/동의 행을 생성하지 않는다.

응답은 version·현재 account_id·observed_at·nullable revision과 기존 내 정보의 consent projection이다. subject/session ID·hash·credential·TC 원문은 내보내지 않는다. unknown/current/expired·현재 목적 선택·시각을 같은 Rust projection에서 만들고 만료를 허용으로 표시하지 않는다. `cmp_available`은 실제 CMP 미연결인 이번 main/fixture에서 계속 false다. 이 조회/응답은 선택 수집 권한이 아니며 at-write는 별도 현재 revision 검사가 필요하다.

## 저장·철회·실패

기존 PgConsentStore를 port로 사용한다. account→session 잠금·현재 token/account/session revision·시계/기한 검증을 유지한다. 재검증은 기존 행의 두 목적을 false·새 revision으로 먼저 commit하고 검증 IO를 잠금 밖에서 실행한다. 늦은 결과는 차단 revision CAS가 일치할 때만 저장한다. 실패/timeout/caller취소는 이전 grant를 복원하지 않는다. 목적별 철회는 다른 목적의 원래 기한을 연장하지 않고, 전체 철회는 한 transaction/새 revision으로 두 목적을 차단한다. 최초 철회도 차단 행을 남겨 늦은 최초 grant를 거절한다.

main과 실제 HTTPS fixture는 같은 PgConsentStore·DisabledConsentVerifier·기존 me 권위 registry/인증 invalidator를 사용한다. 실제 CMP 없이 refresh는503이고, 이미 있던 허용을 재검증하려다 실패해도 사전 차단은 남는다. 테스트의 허용 verifier는 단위/실PG 준비에만 주입하며 제품/fixture 공개 경로에서 가짜 grant를 만들지 않는다. 인증 미설정 합성은 세 경로 모두503/no-store다.

전체 IO2초·모듈 동시4요청·기존 공유 me 권위64개·session/account 각각초당20회·rate key4096/60초를 적용한다. 새 DB pool/CPU 고비용 작업을 추가하지 않는다. 요청 전후 같은 세션과 시계·세션 기한을 검사하고 기존 auth 철회 barrier 아래에서 응답을 만든다. 응답은 고정 크기 필드/최대4KiB이며 no-store/no-referrer/nosniff다. 다른 시점의 read snapshot을 수집 권한으로 소비하지 않는다.

## 검증·후속

행동 Red→Green: DTO 폐쇄/크기/version/UUID, 인증 없는 일반 조회·닉네임 없음/301초 조회·CSRF/소유권/세션 교체·만료/역행/철회/취소·동시/속도 한도·no-store·비밀 제외, Disabled refresh 사전 차단·늦은 CAS·전체 철회 원자성·logout/회전/삭제 cascade를 단위/실PG/HTTPS에서 확인한다. 기존 consent domain95%·전체80%와 원래 browser timeout/retry를 유지한다.

선택 이벤트의 같은 저장 transaction 내 현재 동의 권한/기한·목적 검사와 저장 adapter는 이 이슈 병합 뒤 별도 하위로 진행한다. 실제 CMP/TCF·지역/정책·익명 subject·외부147·상위21 전체·83 해결은 이번 완료가 아니다. 파일 기능을 되살리지 않고 게임/광고를 활성화하지 않는다.
