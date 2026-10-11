# ADR0052 · 게시된 데일리와 첫 검증 완료 저장

- 상태: 채택 · 2026-10-11 · M3 #112 / 상위 #19
- 선행: #109/PR111 develop cbd3541 병합·109 CLOSED 확인
- 추적: FR09/10 · NFR01/02/05/07/08 · TS17/18/20/21/31/35/36

## 게시 권위와 입력 결합

실제 PostgreSQL의 daily_definitions에 UTC date+seed version unique와 검증된 rules snapshot을 게시한다. 공개 seed/metadata는 같은 Rust DailyMetadata로 유도하며 불필요하게 중복 저장하지 않는다. canonical metadata.id를 PK로 사용한다. 게시 후 모든 UPDATE를 거절한다. 동일 날짜/버전의 다른 hash를 요청해도 이미 committed된 snapshot을 반환하며 현재 bundled rules로 교체하지 않는다. 저장된 snapshot/ID는 읽을 때 strict 검증하고 불일치는 Unavailable로 닫는다. 현재 지원 알고리즘과 wire 상한만 허용하며 임의 과거 알고리즘 호환을 주장하지 않는다.

검증기가 실제 decode한 typed replay를 canonical JSON으로 만들고, 실제 Clear 뒤 semantic ReplayOutcome과 canonical payload/32-byte SHA256 digest를 하나의 sealed ReplayEvidence로 묶는다. 필드는 private이고 읽기 전용 접근만 제공한다. DB port는 이 evidence만 받고 별도 request 원문을 받지 않는다. 실제 재생한 입력과 저장할 입력을 worker 밖에서 다시 결합하지 않는다. semantic outcome 비교와 payload identity를 구분한다. 같은 통계를 만든 다른 replay도 다른 제출이며 JSON 공백/키 순서 차이는 같은 canonical digest다. 기존 CPU2/2초·actual permit 수명은 유지한다.

## 첫 완료와 재전송

daily_completions는 account+daily unique와 account+attempt UUID unique를 가진다. 첫 유효 완료 한 건만 저장한다. 같은 account/attempt의 같은 daily+canonical digest 재전송은 같은 공식 receipt를 반환하며 시각/만료/내용을 갱신하지 않는다. 같은 attempt의 다른 입력/날짜는 Conflict다. 첫 완료 후 다른 attempt의 유효 완료는 기존 receipt를 포함한 Practice로 구분하고 새 row/replay를 저장하지 않는다. 다른 계정의 같은 attempt UUID는 독립적이다. 실패 입력과 무계정 연습은 저장하지 않는다.

write는 실제 auth account와 session을 기존 postgres_accounts::authorize의 동일 transaction에서 잠근다. private helper를 crate 내부로만 공유하며 provider·권한·최근 인증 정책은 바꾸지 않는다. repository의 trusted AuthClock에서 현재 시각을 정하고, 닉네임 없는 계정/소멸·만료·철회 session을 거절한다. 검증 중 logout/삭제가 끝나면 저장할 수 없고, 저장 transaction과 logout/삭제가 겹치면 account/session lock 순서로 선형화한다. account FK ON DELETE CASCADE로 완료 row/replay/digest를 제거하며 retry는 계정을 만들지 않는다. 본인 조회도 같은 권한 검증을 사용한다.

publish/read/write 전체는 pool acquire/SQL/commit을 포함한2초 기한을 가진다. commit 결과가 응답 기한과 경합하면 Unavailable일 수 있으며 같은 attempt의 재전송으로 이미 committed된 receipt를 찾는다. 추측으로 새 공식 기록을 만들거나 부분 write를 성공으로 반환하지 않는다. 실제 unique/transaction/SQL lock·rollback·동시 제출을 검사한다.

## 보존과 시간 신뢰

새 첫 완료는 UTC date 시작부터7일의 검증 창 안에서만 받는다. 이미 저장된 같은 attempt의 재전송은 기록의90일 수명 안에서 원래 receipt를 반환할 수 있으며 새 검증 창이나 보존을 연장하지 않는다. publication보다 과거인 서버 clock·미래 완료/잘못된 row는 fail-closed다. 원래 completed_at이 replay7일/공식기록90일 보존 anchor다. 기간은 elapsed seconds로 계산하여 DB timezone/DST 변화가 연장하지 못하게 한다. 논리 본인 조회는90일 정확 경계에서 숨긴다. raw replay는7일 용도만 있으며 실제 물리 삭제/백업 정리는 #25의 출시 조건이다.

receipt에는 metadata/완료 UUID·attempt/본인 진행·오류/개인 replay elapsed/서버 completed_at만 있다. replay elapsed는 client 입력의 참고값이다. 이 foundation에는 verified_time/공식 시간 저장 API가 없고 모든 완료는 completion 검증 수준이다. server ticket/heartbeat·두 순위·HTTP·8언어 웹은 다음 #19 하위다.

현재 AccountExport는 auth 정보만 포함한다. 새 게임기록 내보내기는 다음 필수 HTTP/rights 작업에서 실제 DB·세션 proof와 연결한다. 그 전에는 main/HTTP에 새 수집을 켜지 않는다. 이 작업은 FK 삭제를 실제 검증하며 사용자 공개 동작이나 계정 권한을 추가하지 않는다. 외부OAuth42/운영 성능26·사람27/물리삭제25는 별도다.

## 검증과 완료

DB 시각은 finite·정수 초로 제한한다. 저장된 완료가 게시 시각 이전이거나 검증 창 밖이면 유효한 영수증으로 읽지 않는다. account/session 잠금 대기가 끝난 뒤 trusted clock을 다시 읽고 변경되었으면 같은 잠금 안에서 세션을 재검증한다. 시계 역행은 거절하며 대기 중7일 경계나 session 만료를 넘긴 요청은 새 기록을 만들지 않는다.

실제 canonical payload binding과 DB publish의 행동 Red→Green을 확인한다. 서버 요청 snapshot과 accepted snapshot이 다른 경우, 두 publisher/두 제출의 동시성, canonical retry/changed payload·날짜/서로 다른 계정·삭제/철회/SQL lock/실패 rollback, exact7일·90일/DST/anchor 불변/forward migration·raw replay 크기와 비밀 DTO를 검사한다. tests/unit과 실제PG를 사용하고 Daily domain95%·서버80%/전체 회귀·docs/Mermaid·correctness 리뷰/최신CI를 확인한다. develop squash merge·112 CLOSED 뒤 다음 HTTP/timing/rights 하위를 시작하며 상위19는 OPEN이다.
