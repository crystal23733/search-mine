# ADR0067 · 권위 NSM/D7의 완결된 PostgreSQL snapshot

- 상태: 채택 · 승인된 FR01/16·상위20/하위143
- 선행: #140 CLOSED·PR142/develop b179eb5 병합을 제품 시작 전에 실제 확인했다.
- 요구/시나리오: NFR01/02/05/07/08 · TS20/21/29/31/34/35/36

## 첫 정상 날짜의 근거

30일 raw의 earliest는 생애 첫 정상 게임의 근거가 아니다. 새 평생 flag나 개인별 first marker를 저장하지 않는다. D7 cohort C에 정상 raw가 있는 살아 있는 계정을 candidate로 삼고 같은 RR snapshot에서 기존 90일 정상 online source를 조회한다. 정상은 기존 Rust의 Clear/Timeout뿐이며 튜토리얼/중단/데일리를 섞지 않는다. C보다 앞선 유효 정상 source가 있으면 해당 계정은 C의 멤버가 아님이 증명된다. 앞선 근거가 없을 때 account.created_at가 now-90일보다 엄격히 뒤에 있어 계정 전체 수명이 원본 보존 범위 안이고, 최초 유효 정상 source의 날짜가 C일 때만 명시 FirstNormalCompletion을 만든다. 오래된 계정의 이력 gap·모순은 Incomplete/Conflict로 거절하고0/0%로 저장하지 않는다.

원본 보존은 recorded_at90일이 아니라 COALESCE(retention_started_at,recorded_at) 기준이다. account 생성 전 admission/완료, 소수초, 미래/잘못된 원본, 중복 seat·서로 다른 source/raw 사실을 검증한다. 만료 원본은 첫 정상 근거로 재사용하지 않는다. 검증한 C 멤버의 반환일 raw만 기존 pure d7_cohort에 전달하므로 다른 계정의 임의 first 날짜를 만들지 않는다. NSM도 기존 pure weekly_nsm을 사용한다.

## 수집 범위와 snapshot

개인 식별자가 없는 singleton의 최초 collection_started_at와 coverage_since만 추가한다. 실제 main의 single owner claim/recovery 뒤 Serving 전에 AuthClock으로 최초 초기화하고 재시작 시 원래 시각을 유지한다. 시작 전 기록은 존재해도 완전한 수집으로 주장하지 않는다. Clock 범위/역행, marker 누락·미래, period start<coverage_since 또는 start<=now-raw30일이면 Incomplete다. 정확30일 raw는 이미 논리 만료하므로 완결된 기간으로 계산하지 않는다.

pool 포함 총2초 READ ONLY REPEATABLE READ에서 raw·계정·원본을 관찰한다. pool 대기와 각 SQL의 남은 deadline을 공유하는 transaction-local statement_timeout, 시작/획득/마지막 IO 뒤 Clock 검증을 적용한다. 각 조회는 LIMIT10001로10000 한계를 감지한다. 취소/Clock/SQL/capacity 오류를 빈 성공으로 바꾸지 않는다. 개인 계정 삭제는 snapshot 시점의 살아 있는 계정에서 제외되며 동시 삭제는 RR 일관성을 따른다. 반환 DTO는 UTC period/observed/complete·participants/repeat/human/bot 또는 cohort/return_day/sample/nullable retained/rate/mature뿐이다.

현재 존재하는 자동 source producer는 원본과 raw를 같은 transaction에 commit한다. raw30일 정리는 계산 시 cutoff로 거절한다. source90일 physical cleanup은 아직25의 후속이다. 극단적인 오래된 journal 종료 등으로 source90 만료가 raw30보다 이를 수 있으므로 미래 source cleanup이 미만료 raw를 cascade할 때는 같은 transaction에서 coverage_since를 affected occurred_at+1 이상으로 전진시켜 해당 기간을 Incomplete로 만들어야 한다. account 삭제와 비계정 data loss를 구분한다. 임의 관리자 SQL 손상까지 완결성을 보장한다고 주장하지 않는다.

## 경계와 검증

공개 HTTP admin, 외부 분석 SaaS, 개인 first marker/평생 flag는 만들지 않는다. minimum serving 권한은 collection SELECT/INSERT만이며 runtime 초기화에 DDL/migration 권한을 요구하지 않는다. 이 단계는 snapshot port이며 익명 scalar 저장/calendar13개월 물리보존/자동 materialization/로컬 운영조회는 다음 순차 하위에서 완료한다. 개인 권리는 기존 source/raw export·계정삭제로 충족하고 새로운 개인 데이터는 없다.

실제 PG Red→Green으로 정상NSM/사람·봇/abort, D7 신규·반환일·정확maturity·표본0, 앞선 oldsource nonmember와 history gap, 소스90/30일/collection/Clock, 10001 capacity, RR concurrent account deletion·pool/SQL timeout/cancel/GUC, 최소 DML·실제 main 시작을 확인한다. domain95/전체80·full·docs/Mermaid·실제 코드리뷰·필수6CI·정확head squash merge·CLOSED를 기록한다. 이 snapshot 출구만으로20/M3나 외부42/25/26/27/83/141을 닫지 않는다.
