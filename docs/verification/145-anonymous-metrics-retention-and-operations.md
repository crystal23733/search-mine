# #145 · 익명 집계 보존과 운영 검증

FR01/16 · NFR01/02/05/07/08 · TS20/21/29/31/34/35/36. 선행143 CLOSED·PR144/develop1e58969를 확인하고 [ADR0068](../adr/0068-anonymous-metrics-retention-and-operations.md)을 코드보다 먼저 작성했다.

시나리오는 same-RR anonymous materialization·원래UTC period의13달력개월·월말/exactexpiry·불변/동시/equalClock·Incomplete/Capacity 비덮어쓰기·현재계정삭제/과거anonymous·물리500+1/잠금/Clock/cancel·최소DML·main/CLI·maintenance cadence/stale/nooverlap와상위20 통합 인수 감사다.

각 full은 새 검증 DB를 사용하며 이전 DB/실패 trace를 보존한다. PR/최신CI/정확head 병합/CLOSED는 실제 수행 뒤 기록한다.

#145 실제PG 익명 저장 Red0pass/1fail/0.60초(Unavailable)→Green1pass/0.74초. counter 관계의 새 unit Red(게임3·계정2·repeat0 허용)를 확인해 수정하고3unit pass. worker Red는현재주실패때두대상호출이0이었고 Green5개로분리/23slot을확인했다. 실제PG13개월 물리삭제 Red0pass/1fail/0.55초→Green2pass/0.68초;2026-01-31→2027-02-28 원래expiry·재계산 비연장·논리exact만료/물리DELETE를확인했다. port budget/즉시60초/skip/23slot/4초/겹침0/잘못된period·Clock까지8unit pass. clippy의collapsible_if/needless_borrow는수정했으며최종clippy는후속재실행한다. 운영CLI준비의private필드컴파일오류는행동Red아님;수정뒤실제프로세스CLI Red0pass/1fail/0.46초로저장보고서조회실패를확인했다. CLI Green/추가PG/전체/fresh/CI/리뷰·병합은아직전이다. 기존detachedowner1+routepool5+maintenancepool2로최대8연결을실제소스와대조해설계수를바로잡았다. logs .tmp/events145-*는timestamp별보존하고CLI전실패도보존한다.

## 실제 코드 리뷰와 후속 검증

기준 develop1e58969의 diff와 새 파일을 대조했다. 원래 period/expiry, RR source/raw/account와 저장의 원자성, SQL trigger/최소 DML, 동시·equal Clock 충돌, 취소/rollback/공유 deadline, 계정 삭제와 과거 익명 집계, main의 독립 worker/최대8연결, CLI 오류·nullable 비율·stale를 검토했다.

리뷰에서 정상 전진 Clock의 동시 보고서를 IO 전 시각으로 거절하는 결함을 찾았다. 실제 PG Red0pass/1fail에서 저장 computed=now+1·마지막 Clock=now+1을 거절하는 것을 확인하고 마지막 IO 뒤 Clock으로 검사하도록 수정했다. 실제 PG12개/4.06초 통과로 정상 전진·실제 미래·exact13개월 마지막 IO 만료를 확인했다. CLI 출력 직전 만료도 unit Red0pass/1fail 뒤 available 대신 expired를 표시하도록 수정하고1pass로 확인했다. counter 관계의 누락은 위 Red로 수정했다. 이후 검토에서 추가로 확정한 결함은 없었다.

최신 all-targets clippy12.93초·문서194개·diff whitespace 검사를 통과했다. 독립 DB liar_events145full_20261016202042의 전체 실제 PostgreSQL17.4 회귀148개/31.93초 통과. 전체 웹/E2E·fresh coverage·원격 PostgreSQL18/필수6checks 결과는 후속 기록한다. 캐시 공간 확보는 이전 #103/#105의 재생성 가능한 debug 산출물만 삭제했고 DB·로그·coverage 보고서는 보존했다.

## 상위 #20 통합 인수 감사

| 인수 조건 | 실제 구현과 근거 | 범위와 종료 판단 |
|---|---|---|
| event_id 중복 제거·purpose/권한 | #134 closed client9/server2·목적별 consent 도메인; #136 DB essential CHECK·불변 ID/source dedup·현재 account 잠금; #138 source와 event 같은 transaction | 필수 match_end/daily_verified만 자동 수집한다. optional runtime은 차단하며 실제 consent subject/revision의 원자적 at-write 검사는 #21 연결 때 구현한다. 이를 구현 완료로 계산하지 않는다. |
| raw30일·anonymous13개월 | #140 원래 발생시각30일·500/SKIP LOCKED/2초; #145 원래 period13달력개월·논리 exact 만료와 물리500/SKIP LOCKED/2초 | cleanup 지연 시에도 논리 만료를 지킨다. 운영 처리량/RPO/RTO는 #25/#26의 실제 환경 출구다. |
| authoritative NSM/D7 | #143 기존 source90일 first-normal 증명·old gap 거절·collection/RR/10000상한; #145 같은 RR 저장·자동 주기·로컬 CLI·표본/nullable/시각/stale | 전체 목표 표의 funnel·광고·큐/시작/추궁 지표와 실사용자 목표 달성을 주장하지 않는다. 현재주60초, 이전주/최근 mature D7 약23분 주기다. |
| 본인 export·삭제 | #136/#138 현재 proof의 본인 raw 이벤트와 실제 HTTPS 다운로드; #143/#145 RR 계정 삭제 반영 | 기존 익명 보고서에는 개인 명단/mapping이 없고 원래13개월 보존한다. 미래 #25의 source 삭제가 미만료 raw를 cascade하면 같은 transaction에서 coverage_since를 전진시켜야 한다. |
| 검증·리뷰·병합 | #134/136/138/140/143의 PR135/137/139/142/144 병합과 CLOSED 확인; #145의 실제 Red/Green·회귀·리뷰는 위 기록 | #145 최신 전체/fresh/필수6checks·정확head develop 병합·CLOSED 확인 뒤에만 #20을 닫는다. M3/#15와 #42/#25/#26/#27/#83/#141은 별도다. |

#20의 원래 백로그 계약은 dedup·목적 분리·30일/13개월·NSM/코호트/삭제다. #21이 #20을 선행으로 두므로 실제 CMP/선택 분석 활성화는 다음 이슈의 책임이며 현재 차단 상태가 이 단계의 출구다. 기획의 전체 목표와 현재 구현을 구분한 채 #145 병합 전까지 #20 OPEN을 유지한다.

## 최종 로컬 검증

최종 로컬 같은 제품 source: 전체 scripts/check.ps1 exit0, Rust429pass·웹267pass/90.91%·Chromium113pass/6.5m/재시도0·WASM/types/fixtures/tokens/format/lint/typecheck/build·문서194/Mermaid34failure0 통과. 독립 DB liar_events145full_20261016202042의 실제PG17.4 회귀148/31.93초. fresh coverage 472pass(실제PG148/37.90초), Event478/479=99.79%·Privacy100%·서버10984/11439=96.02%로 모든domain95/전체80 게이트 통과; 실제 metrics/main/migrate entry 포함. 전체 로그 events145-validation-20261016202042, fresh 보고서 events145-coverage-20261016202552.json 보존. 원격PG18/최신6checks·정확head 병합/CLOSED는 다음 단계다.


## 원격 인수 실패와 기존 데일리 동시 발행 수정

최초 source611d949의 Product38894668896 database는147pass/1fail/51.35초였다. daily start 동시 최초 발행의HTTP503이며 PG로그의동일daily ID primary key 중복을 실제 확인했다. 날짜/버전만 conflict target인 기존 publish를 모든 unique 충돌 DO NOTHING 뒤 typed accepted snapshot 조회로 보정했다. ADR0068을 코드보다 먼저 갱신했고 같은발행4개/16날짜의 실제PG Red0pass/1fail/0.64초(Unavailable)를 관측했다. 첫 Red도0/1/.97초였으나 반복version2 이후가 유효하지 않아 날짜 반복으로 고친 뒤 다시Red를확인했다. 첫실패raw로그events145-ci-first-database.log와Red로그는보존한다. 수정후전체PG149/full/fresh/새head6CI를재실행하며이전source의성공을새source통과로대체하지않는다.

수정 뒤 실제 PG 데일리26개/9.29초 Green으로 동일 발행4개·16날짜/다른 snapshot 경합·accepted evidence·최초 기록/재시도·손상/권한/삭제/시각 경계를 확인했다. SQL 변경은 모든 unique 충돌을 skip한 뒤 기존 typed 조회를 유지하므로 손상·다른 날짜를 승인하지 않고 기존 값·published_at도 덮지 않는다. 이후 정확 source의 전체/fresh/최신6checks·병합/CLOSED 결과는 [PR146 최종 검증 기록](https://github.com/crystal23733/search-mine/pull/146)과 로컬 당일 review에 이어 기록한다.
