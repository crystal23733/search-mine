# ADR0054 · 인증 데일리 완료 HTTP와 응답 권위

- 상태: 채택 · 2026-10-11 · M3 #116 / 상위 #19
- 선행: #114/PR115 develop04fe9d1 병합·114 CLOSED 확인
- 추적: FR01/09/10/16 · NFR01/02/05/07/08 · TS17/18/20/21/31/35/36

## 공개 계약과 신뢰

고정 POST `/api/v1/daily/definition`, `/api/v1/daily/submit`, `/api/v1/daily/own`만 제공한다. Rust 원천 closed DTO를 TS로 생성한다. 공통 요청은 v1·canonical non-nil account UUID·현재 공개 session_revision UUID다. definition은 date/seed_version, submit은 attempt UUID/replay, own은 metadata를 더한다. URL에 개인정보·attempt/replay를 넣지 않는다. Origin·메모리 CSRF·HttpOnly 세션 쿠키·단일 JSON Content-Type·query 없음이 필수다. 닉네임을 설정한 현재 계정만 공식 기록을 사용할 수 있으며300초 재인증은 요구하지 않는다. 계정 권리 #114의 최근 인증 정책은 그대로 둔다.

definition 응답은 accepted metadata/rules snapshot, submit 응답은 account/요청 attempt·Official 또는 Practice·최소 원래 receipt, own 응답은 본인 receipt 또는 정상 null이다. receipt에는 완료 UUID·원래 attempt·metadata·본인 진행/오류·개인 replay elapsed·서버 완료 시각만 있다. trust는 `verified_completion` 하나다. 공식 시간 필드를 만들지 않으며 server ticket/heartbeat·verified_time/두 순위·8언어 명시 제출 UI는 후속이다. 규칙 일관성 검증은 사람 풀이·오프라인 경과시간의 증거가 아니다.

## 게시와 저장 권위

DailyService는 repository의 committed definition을 먼저 조회하고 반환된 snapshot을 공유 Rust 검증기에 전달한다. 없다면 요청 date/version으로 **서버 bundled rules**에서 candidate를 만든 뒤 publish가 반환한 accepted snapshot을 사용한다. 클라이언트가 rules를 제출하지 않는다. 기존 PG publication의 UTC7일 창과 불변 unique가 cold-start/오프라인 날짜·두 publisher의 경쟁을 결정한다. 기간 밖의 새 게시·지원하지 않는 버전은 거절한다. 이미 게시된 과거 snapshot을 현재 bundled로 바꾸지 않는다. 미래·만료를 로컬 시간으로 추정해 수집하지 않는다.

submit은 strict replay를 실제 bounded native worker에서 재생해 받은 sealed canonical evidence만 기존 repository에 전달한다. account/session lock·첫 완료 unique·동일 attempt/digest 원래 receipt·다른 attempt의 Practice·7일/90일·삭제 cascade를 재사용한다. own은 accepted metadata가 정확히 같아야 하며 DB 실패를 null로 바꾸지 않는다. 응답 receipt의 원래 metadata/UUID/완료 시각/유효 기간/통계와 Official의 요청 attempt·evidence 통계를 다시 확인한다. Practice는 원래 첫 완료를 반환하므로 새 replay 통계를 섞지 않는다.

검증 port가 성공 evidence를 반환해도 adapter 경계에서 **현재 요청의 canonical typed replay와 accepted metadata에 정확히 결합**됐는지 실제 worker 안에서 확인한 뒤 저장소로 넘긴다. 다른 동시 요청/캐시된 evidence는503이며 write0이다. 의미가 같은 통계만으로 payload가 같다고 판단하지 않는다. 원문의 JSON 공백/키 순서는 canonical typed 직렬화로 정규화한다. 검증 port의 명시 오류는 기존 분류를 유지한다.

## 자원과 수명

submit 본문은 replay2MiB+envelope4096byte, 다른 요청은4096byte다. 큰 JSON의 decode·closed field 검사·replay 추출 및 응답 직렬화는 기존 bounded verifier의 **같은2개 CPU permit**을 공유한다. 정상 호출은 각 작업이 끝난 뒤 다음 단계를 시작한다. 모든 spawn_blocking 작업은 실제 closure가 permit을 소유하며 caller timeout/cancel 뒤 조기 반환하지 않는다. 각 CPU 작업2초, HTTP는 최초 실제 session proof부터 decode/PG/replay/직렬화/최종 proof까지 전체2초다. PG의 자체2초 기한을 늘리지 않는다. 입력 수·native core 규칙은 Rust 기존 검증기를 사용한다.

동시 요청 기본4(1..64), authority 기본64(1..20000), 세션/계정20회/초와 bounded rate key를 적용한다. `LIAR_DAILY_REQUESTS`, `LIAR_DAILY_AUTHORITIES`를 strict 숫자로 읽는다. busy·시간초과·오염·저장 장애는503, rate는429다. malformed/지원 버전/규칙 불일치/미완료는400, attempt 충돌은409, 검증/기록 만료는410, 미인증/닉네임 미설정/철회는401이다. 모든 응답은 no-store/no-referrer/nosniff다. 키 없는 구성은 같은 고정 경로503이며 계정을 만들지 않는다.

## 세션과 commit

세션 registry generation을 최초 실제 세션 조회 전에 얻는다. 첫 세션의 ID/account/불변 시각/nickname/만료를 확인하고 요청 account/revision과 일치시켜 별도 daily authority를 bind한다. 모든 주요 await 이후 trusted clock의 이전 관측값보다 역행하지 않았는지 확인한다. 저장/직렬화 후 같은 hash의 실제 세션을 다시 읽고 ID/account/created/authenticated/expires가 동일하며 현재 유효한지 확인한다. revoke 감시와 최종 `with_authority` 장벽 안에서 이미 만든 응답만 commit한다. auth logout/계정 삭제/연결 변경 invalidator에 이 registry를 연결한다.

DB commit이 응답 기한/세션 철회와 경합하면 성공 응답이 없어도 저장됐을 수 있다. 원래 attempt로 재전송하여 receipt를 찾는다. 응답 취소가 이미 committed된 기록을 롤백했다고 주장하지 않는다. 삭제가 먼저 끝나면 PG의 실제 account/session lock은 기록을 복원하지 않는다. 닉네임 변경은 신원/세션 revision을 대신하지 않는다.

## 검증과 다음 작업

실제 route 미연결 Red→Green, parser/proof/receipt 정책·port 오류·정상 null/Practice/재전송·metadata/owner/revision·clock 역행/기한/동시/rate·실제 worker 수명을 검사한다. 실제PG를 사용하는 HTTP로 게시/첫 완료·삭제/재전송/조회/내보내기 연결을 검사하고 실제 HTTPS fixture도 같은 production router/PG/native 검증기를 사용한다. 외부 OAuth proof만 fixture이며 실제 키 검수는42다. domain95%/서버80%·전체 회귀·docs/Mermaid·correctness 변경 리뷰·최신CI 후 squash 병합·116 CLOSED를 확인한다. #19·물리정리/백업25·성능26·사람27·미해결SW83은 열어둔다.
