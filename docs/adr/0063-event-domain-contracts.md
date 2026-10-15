# ADR0063 · 목적·보존·NSM/D7 이벤트 도메인 계약

- 상태: 채택 · 승인된 FR16/#20의 하위 #134
- 선행: #19 CLOSED2026-10-13T15:02:05Z, PR133/develop0d25f9f 병합 실제 확인
- 요구: FR16, NFR01/02/05/07/08 · TS20/21/22/24/29/31/34/35/36

## 결정과 경계

`events` Rust 도메인을 model/policy/metrics 책임으로 분리한다. 이 단계는 저장·수집 이전 계약이며 PostgreSQL/HTTP/main/client/CMP를 켜지 않는다. 새 의존성·migration·public wire/생성 TS를 추가하지 않는다. 이후 public API를 만들 때 별도 Rust protocol DTO에서 생성한다. 게임 규칙은 기존 Rust 코어와 서버 기록이 권위이며 client 이벤트를 승패·정산·NSM 증거로 사용하지 않는다.

client draft는 event_version1·canonical nonnil UUID event_id/탭 내 일시 session_key·UTC occurred_at·8locale·닫힌 event enum만 받는다. 최대1024 UTF-8 byte, 알려진 필드/종류/숫자·날짜/enum만 허용하며 임의 JSON/URL query/닉네임·OAuth·IP·쿠키·토큰·정답/seed·좌표/입력은 받지 않는다. server/essential/account/purpose/consent를 client가 지정할 필드는 없다. server match_end/daily_verified는 별도 trusted Rust 생성 경계다. 서버 계정 ID와 opaque tab correlation을 구분하며 auth session ID/토큰을 복사하지 않는다. 실제 서버 수집은 후속의 committed game fact 결합 뒤에만 활성화한다.

landing/invite/tutorial/rematch/share는 analytics, ad request/impression/failure는 advertising이다. 목적별 내부 consent snapshot의 유효 granted/revision·시작/만료가 사건 발생과 현재 시각을 모두 덮어야 선택 이벤트를 허용한다. 다른 목적의 허용, unknown/denied/error, 발생 뒤 동의, 만료·시계 rollback·nil revision은 거절한다. client가 보낸 광고 이벤트도 불신 claim이며 후속 실제 SDK callback producer와 구분한다. snapshot은 저장 권한 proof가 아니므로 후속 PG write는 현재 동의 revision·권한을 같은 transaction에서 재검증해야 한다. #21 전 비필수 runtime 수집은 기본 차단한다.

불변 normalized event에서 같은 event_id+같은 전체 내용은 duplicate, 같은 ID의 다른 목적/actor/시각/내용은 conflict, 다른 ID는 new다. 원래 occurred_at을 retry로 바꾸지 않는다. 원시 보존은 원래 발생에서 정확30×86400초 미만, 집계는 UTC 날짜 시작에서 calendar13개월 미만이다. 월말은 chrono의 달력 clamp(예: 2024-01-31→2025-02-28), 정확 만료/음수·역행·범위/overflow는 실패로 구분한다. 익명 집계는 계정/session/event/match ID 없는 counts·기간·완결성만 반환하며 실제 삭제/물리 보존은 후속 저장/retention에서 검증한다. 이 기간을 법정 의무나 법적 검수 완료로 주장하지 않는다.

## 지표의 권위와 완결성

NSM은 [기존 정의](../product/09-north-star.md)의 UTC월요일 주간 정상 clear/timeout 1:1 distinct(account,match)≥2 계정 수다. 봇 상대 포함, client·daily·forfeit/abandoned/server_failure/cancelled 제외다. `PublicEndReason.completed()`는 forfeit도 포함하므로 NSM의 정상 완료 predicate로 대체하지 않는다. 상대 bot/human은 매치 참여 횟수로 분해하며 사람 대전 두 계정을 경기 한 건으로 잘못 표시하지 않는다. 같은 match의 양쪽 계정은 날짜/종료 이유/상대 종류가 일관돼야 한다. 삭제 계정 입력 제거 후 다시 계산하면 그 계정의 식별 지표는 사라진다.

D7은 저장 계층이 제공할 **명시 authoritative 첫 정상 완료 날짜**의 모집단과 정확 UTC+7일 정상 활동을 결합한다. 보존된30일 이벤트 중 첫 관측을 평생 첫 판으로 추정하지 않는다. +7일이 아직 끝나지 않았으면 retained=null·mature=false, 분모0의 rate도 null로 표시한다. 부모 cohort의 모집단/활동 snapshot이 불완전하면 계산을 거절한다. 두 지표는 각 입력 최대10000, 오염/동일 ID의 다른 내용·동일 account/match의 충돌·중복 첫 날짜를 실패로 노출한다. 상한 초과를 잘라 완전한 표본처럼 보고하지 않는다. 기간 밖 입력도 구조와 identity를 검증한다.

## 검증과 다음 순서

검토 보강: consent snapshot은 승인된 opaque session subject를 포함하고 draft의 session_key와 같아야 한다. 다른 세션의 유효 동의를 재사용할 수 없다. ClientDraft는 공개 Deserialize를 제공하지 않고 길이·버전·locale·시각·종류별 값 검증을 마친 decoder만 생성한다. snapshot의 subject/revision을 caller가 작성할 수 있으므로 이것도 저장 권한 증명은 아니며 실제 저장 adapter의 현재 권한 재검증 의무는 유지한다.

실행 가능한 행동 Red→Green으로 client 서버권위/추가 민감필드 거절, 목적별 동의·발생/현재시각, identity 충돌, 30일/달력13개월·윤년·DST/overflow, 정상/포기 분리·양쪽 참가자·중복/부분주, D7 첫 날짜/정확+7일/미성숙/분모0/삭제·오염·상한을 검사한다. event model/policy/metrics 모두 domain95 gate 필수 파일로 포함하고 누락 보고서가 실패하는지 확인한다. full·실제 기존 DB 회귀·문서/Mermaid·코드 리뷰·최신CI/정확head 병합·CLOSED 후 다음 #20 하위를 시작한다.

필수 후속은 PostgreSQL event_id dedup·원자적 동의/계정 권한·본인 export/삭제, committed online/daily server producer/운영 조립, bounded retention·NSM/D7 first anchor/일별 집계·운영 조회, #21의 실제 consent/client 연결이다. 이 foundation으로 #20/M3 전체나 수집·삭제·실제 이용 지표를 완료했다고 보고하지 않는다. 외부42·운영25/26·사람27·미확정83은 별도다.
