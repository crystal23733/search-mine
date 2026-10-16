# #143 · 권위 NSM/D7 snapshot 검증

FR01/16 · NFR01/02/05/07/08 · TS20/21/29/31/34/35/36. [ADR0067](../adr/0067-authoritative-metric-snapshots.md)을 코드보다 먼저 작성했다. 선행140의 PR142 develop 병합과 CLOSED를 확인한 뒤 시작한다.

시나리오: raw min을 first로 추정하지 않으며 같은 RR에서 전체 수명90일 또는 earlier authoritative source로 Known/nonmember를 증명한다. old gap/collection 이전/30일 만료/Clock/capacity는 빈 성공으로 바꾸지 않는다. NSM은 정상 사람/봇, D7은 C+7 UTC일·exact maturity·표본0/nullable rate·현재 삭제를 확인한다.

실행 결과·Red→Green·코드 리뷰·커버리지·원격검사/병합 상태는 수행 뒤 실제로 기록한다. 아직 통과나 완료를 주장하지 않는다. 익명 저장/13개월/운영조회/20 감사는 다음 순차 하위다.

실제 PostgreSQL Red0pass/1fail/0.43초: 정상 source 두 번과 D7 반환을 저장한 뒤 weekly가 Unavailable이었다. Green1pass/0.67초로 NSM repeat1·bot2 및 D7 sample1/retained1을 확인했다. 최초 E0583 테스트 모듈 경로 오류는 행동 Red로 계산하지 않는다.

경계 첫 실행6pass/1fail/5.55초는 과거 시각 조회 전 다음 날 source를 저장한 fixture 오류였다. 미래 기록을 거절한 동작과 구분해 시간 순서를 수정했다. 뒤이어8pass/5.59초, 최소 serving DML/main restart 포함10pass/5.72초였다. NSM 사람·봇/abort 제외, exact D7 maturity·nullable rate/0sample, old gap Incomplete/earlier source nonmember·원래90일 보존, collection/원래30일/Clock·source 모순, RR 동시 계정삭제, pool/SQL deadline·취소/GUC, raw/history10001행·정확10000 raw를 실제 PG에서 검증했다. 제한 역할은 collection SELECT/INSERT만으로 초기화/조회·재시작을 수행하고 UPDATE/DELETE/DDL/migration 읽기를42501로 거절했다.

권위 source 검증을 기존 수집과 재사용하고 계정별 최초 source를 한 번의 정렬 map으로 찾도록 정리했다. 같은 소스 전체 PostgreSQL136pass/22.41초, all-targets clippy·문서192/diff check를 통과했다. 실행 helper의 빈 ArgumentList 준비 오류는 수정 후 새 timestamp 로그에 남겼으며 제품 실패와 구분했다. full/fresh coverage/원격6checks·정확head병합/CLOSED는 아직 진행 전 또는 진행 중이다.

정확성 changes 리뷰 기준 develop b179eb5: 원자적 producer→같은 RR의 raw/source 사실 대조, original retention→30·90일/첫 정상 근거, account 전체수명과 earlier proof→명시 first/oldgap 거절, pool/SQL/마지막 IO Clock→공유 deadline, 삭제→RR 일관성, owner→최초 marker/재시작, SELECT/INSERT→다른 DML/DDL 차단을 검토했다. 이 범위에 추가로 뒷받침된 제품 결함은 없었다. 새 EventMetricReader domain도95% 게이트의 필수 파일 목록에 포함해 누락을 막는다.

첫 full의 기존 offline-mobile two-tab update/load10000ms 실패를 별도83에 기록했다. 두trueACK/skipWaiting-request 뒤 완료/controllerchange는10초안관측되지않았고 trace·PC/mobile probe2개는 .tmp/events143-offline-failure-20261016022528에 보존했다. #143은 SW/update/activity/timeout/retry 불변이며 성공으로 덮거나 원인 해결을 주장하지 않는다. 비교/기본반복/새full 결과는 수행 뒤 별도로 기록한다.

기존83의 네트워크 계측 비활성 PC/mobile 각4회·총8pass/32.5초, 기본 계측 PC/mobile 각4회·총8pass/34.1초를 같은 소스·retries0/workers2에서 확인했다. 이 성공으로 최초 load 실패 원인 해결을 주장하지 않는다. 최초 full110pass/1fail/2미실행/5.3분과 새 full 검증을 구분한다. SW/update/activity/timeout/retry는 변경하지 않았다.

최신 동일 소스 fresh coverage는451pass(실제PG136/24.62초), Event338/339=99.71%·Privacy182/182=100%·서버10219/10861 lines = 94.09% (required 80%)로 모든 domain95/전체80 게이트를 통과했다. raw 보고서는 로컬 events143-coverage-20261016023500.json이며 새 EventMetricReader 필수 파일도 포함했다. 새 scripts/check.ps1 full과 원격CI·정확head병합/CLOSED는 아직 진행 전 또는 진행 중이다.
