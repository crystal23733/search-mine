# ADR0064 · 필수 이벤트 PostgreSQL 저장과 본인 권리

- 상태: 채택 · 승인된 FR01/16/#20 하위 #136
- 선행: #134 CLOSED·PR135/develop0155012 병합 확인
- 요구/시나리오: NFR01/02/05/07/08 · TS20/21/29/31/34/35/36

## 저장 권위와 범위

필수 event store는 opaque event ID와 내부 account·온라인 match/데일리 completion ID만 받는다. 현재 PostgreSQL의 committed 본인 인간 참여 결과/데일리 완료에서 내용과 원래 발생시각을 유도한다. client draft나 임의 ProductEvent를 저장할 API는 없다. 도메인은 SQL을 모르고 PostgreSQL adapter가 기존 Rust event model/policy를 사용한다. 자동 runtime producer는 다음 하위이며 이 저장 출구만으로 실제 수집을 주장하지 않는다.

새 forward migration의 product_events는 JSON payload 없이 알려진 최소 scalar만 저장한다. server/essential만 허용하는 purpose CHECK, canonical nonnil event ID, account/source FK 삭제, source당 본인1건과 event ID unique, 발생·수신시각 정수/범위·수신 당시 원래30일 이내, 두 fact의 XOR/enum/숫자/날짜 CHECK 및 불변 update trigger를 둔다. secret seed·보드·입력·IP·OAuth·토큰·nickname을 복사하지 않는다. 과거 migration/checksum은 수정하지 않는다.

같은 transaction에서 살아 있는 account를 먼저 잠가 삭제와 직렬화하고 committed source를 읽는다. 대전의 본인 인간 참여는 정확1건이어야 하고 bot/human 상대는 기존 참여 기록에서 유도한다. 피어 계정 삭제 뒤 남은 인간1명도 human으로 유지한다. 원본 발생·원본 게임 보존·raw30일과 Clock rollback을 검사한다. 전체 작업은2초 timeout이다. 같은 event ID의 다른 account/source/content는 conflict다. 같은 원본을 새 ID로 재시도하면 기존 accepted event ID·received_at을 반환하며 호출자가 제안한 ID를 새로 저장하지 않는다. 원본 만료·삭제 후 재전송은 복원하지 않는다. event ID dedup 수명은 보존 중인 raw이고 영구 식별 tombstone을 만들지 않는다.

analytics/advertising는 write API 부재와 DB essential CHECK로 차단한다. #134의 ConsentSnapshot은 현재 동의 증명이 아니며 선택 이벤트 저장 시 현재 subject/revision을 동일 transaction에서 검증해야 한다는 의무는 #21의 실제 consent adapter 연결 때 적용한다. 이번 변경은 CMP 동의·guest 추적을 만들지 않고 무계정 연습에 계정을 생성하지 않는다.

## 본인 내보내기와 삭제

기존 Rust privacy ExportStream/ExportRecord에 event 스트림과 닫힌 server fact DTO를 추가하고 TS를 원천에서 생성한다. JSON 다운로드의 기존 games 배열은 online/daily/event를 함께 포함한다. event DTO는 event_id·occurred_at·received_at·최소 fact만 포함하고 account/session·consent proof나 game secret을 중복하지 않는다. 웹은 세 스트림을 모두 수집하고 닫힌 필드/ID/enum/시각·정렬/페이지 상한을 검증한다. 기존2048페이지/32MiB·취소/현재 owner·100ms pacing을 유지한다.

기존 명시 내보내기·최근 재인증·Origin/CSRF·현재 account/session revision과 전후 proof·2초IO·50개/페이지·원래 raw30일·역행 검사를 재사용한다. 삭제는 account/source FK cascade로 제거되고 저장 재시도는 account/source 권위 확인 후에만 처리된다. 여러 export 페이지를 단일 DB snapshot으로 주장하지 않는다. 실제 PG 경합/삭제·HTTPS 다운로드·8언어와 기존 게임/replay 출구를 검증한다.

## 검증과 후속

원본 검토 보강: daily event의 trust는 기존 publication/receipt 검증을 통과한 값에서만 유도한다. DB에 있는 Some 시간만으로 verified_time을 만들지 않는다. 기존 본인 Daily 조회가 거절하는 손상된 완료/시간/발행 binding을 이벤트 저장이 정상 사실로 바꾸지 않아야 한다. 신규 게임 규칙을 복제하지 않고 기존 Rust receipt validator를 재사용한다.

실제 DB 행동 Red→Green으로 source 권위, 동시 동일ID/충돌/새ID 원본 dedup, accepted ID/received_at 유지, 삭제 경합·비복원·불변/민감/목적 CHECK, 정확30일·rollback·시간제한·본인51개 페이지/권한/현재 revision을 확인한다. public DTO/웹의 event decode·수집과 실제 HTTPS 파일에 본인 raw가 포함됨을 검증한다. 이벤트/개인정보 domain95·서버80, full/실DB/문서/Mermaid·코드리뷰·필수CI·정확head 병합/CLOSED를 확인한다.

자동 committed producer/runtime, first-normal anchor·NSM/D7 집계 저장, 물리 bounded retention/운영 조회는 필수 후속 #20이다. #21의 실제 consent/CMP, 외부42·운영25/26·사람27·미확정83은 별도다. 저장·권리를 #20/M3 전체 완료로 계산하지 않는다.
