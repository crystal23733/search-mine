# #109 · bounded native 데일리 replay 검증

M3 · FR09/10 · NFR01/02/05/07/08 · TS17/18/20/21/31/35/36.

선행 #107/PR108은2026-10-11T04:21:38Z develop6a2be6d에 병합됐고107 CLOSED04:21:40Z·상위18 CLOSED04:21:56Z를 원격 조회로 확인했다. #109는 [ADR0050](../adr/0050-bounded-native-daily-replay.md)의 native replay/domain/CPU 수명 foundation이다. 설계·추적을 제품 코드 전에 반영했다.

## 실행 예정 시나리오

- 기존 native/WASM golden 완료와 동일 metadata/오류/진행/재생 elapsed, incomplete/timeout 거절.
- strict metadata/shape/version/2MiB/count/time/action/cell/command identity, 정확한 중복과 정상 rejected 클릭 구분.
- 서버 published retained snapshot으로만 재생, current rules fallback·unsupported definition 거절.
- 실제 blocking worker 한도2·fail-fast Busy·2초 caller 기한·timeout/cancel 뒤 permit 유지·실제 종료 후 재사용·panic/late result·size-before-admission.
- 실제 Red/Green·fresh Daily95%/서버80%·관련 Rust/생성물/웹/실DB·문서/Mermaid·최신 CI·코드 리뷰.

위 항목은 시작 시점의 검증 계획이며 실제 실행 결과는 아래에 이어 기록한다. HTTP/PG write/공식 첫 완료/ticket·heartbeat/순위/UI와 상위19 완료를 뜻하지 않는다. 공개 replay 일관성은 인간 플레이나 실제 오프라인 시간을 증명하지 못한다. review/2026-10-11.md는 Git 제외 상태로 이어 쓴다.


## 실제 TDD와 초기 Green

native golden 완료 검사는 아직 Unavailable인 skeleton에서 실제0pass/1fail(Exit101,0.18초)을 확인했다(.tmp/daily109-red.log). shared DailyPuzzle 어댑터와 metadata/shape/버전/2MiB/count/time/action/cell/identity·중복/정상거부를 구현한 뒤8개0.71초PASS. CPU facade도 실제 native evidence 대신Unavailable을 반환하는 skeleton에서0pass/1fail(Exit101,0.30초)을 확인했다(.tmp/daily109-worker-red.log). 실제 spawn_blocking/owned permit·2workers/2초/Busy/size-before-admission을 구현하고 retained snapshot/기절 입력까지 확장한최종15개2.27초PASS(.tmp/daily109-green.log). golden의 own.stats.mistakes 필드 형태를 실제 fixture로 확인하여 테스트 oracle을 정리했다.

완료 evidence는shared core의 Clear/SeatOne만이며 golden own216/mistakes0/replayed3200ms다. 현재값과 다른 보존 snapshot도 실제공유core로완료하고현재rules fallback은거절했다. timeout/cancel에서 실제 두worker입장을확인한후초과Busy·callerTimeout/Join취소뒤Busy·gate release/실제종료뒤capacity재사용을검사했고panic/oversize도검사했다. gate는Drop시반드시release한다. Daily model95% gate를코드에추가했으며현재전체회귀/freshcoverage/리뷰CI/PR/병합은아직미완료다. HTTP/DBwrite/공식시간/순위/UI/상위19완료는없다.


## 최종 domain/실DB coverage와 상관관계 리뷰

최종 신규16개2.27초(실제cov lib36개2.33초)와 실제PG17.4 61개7.64초/ignored0을 확인했다. 이 이슈 전용 새 .tmp/daily109-coverage-clean에서 시작했고, 역순 완료 테스트 추가 뒤 제품 domain/adapter source가 동일한 상태로 최종 보고서 .tmp/daily109-coverage-final.json을 갱신했다. Daily model 파일1개62/62=100%·서버6959/7477=93.07%·Auth99.48/Match99.31/Lobby98.37/Result100%가 모두 gate를 통과했다. 이전 이슈 빌드자료를 재사용하지 않았고 테스트 본문은 tests/unit만 제외했다. fmt/Clippy workspace all-targets PASS·docs159/Mermaid34 PASS.

pm-skills correctness/changes·baseline develop6a2be6d 리뷰에서 server publication과 client proposal7필드 차이·retained hash/현재fallback·core duplicate/seq/conflict/거부클릭을 확인했다. 동시 두 definition의 완료를 역순으로 제어해 각 caller의 서로 다른 오류가 보존되는 추가 실제검사도PASS다. 실제 worker입장뒤 caller취소/Timeout과Busy·실제종료뒤재사용·panic/oversized-before-admission을확인했다. Tokio공식spawn_blocking/owned semaphore/timeout 계약도읽고ADR에연결했다. 현재검토범위에서근거있는미해결correctness결함을확인하지못했다. 임의과거algorithm/7일보존registry/운영성능/공식시간/HTTP·PGwrite는범위밖이며후속19에서구현한다. 전체check/최신CI/PR/병합은아직진행중이다.


## 최종 로컬 전체 검사·코드 리뷰 완료

`scripts/check.ps1`은 Handle을 보존한 실제 프로세스의 Exit0/null거절을 확인했다. fmt/Clippy all-targets·331Rust·WASM/types/fixtures/tokens freshness·format/lint/typecheck·211unit37파일/web88.25%·production build·96browser3.4분재시도0·docs159/Mermaid34실패0 PASS. stdout/stderr는 .tmp/daily109-check.stdout.log/stderr.log다. 별도 실제PG17.4 61개7.64초/ignored0·최종Daily62/62 100%/서버6959/7477 93.07%와 각기존domain95이상 보고서를 구분한다. 도메인 누락 보고서를 실제gate가Exit1로거절한것도확인했다.

만든 코드: `crates/server/src/daily/model.rs`는 PublishedDaily/입력검증·완료 evidence·오류/port, `daily/replay.rs`는 strict protocol→shared DailyPuzzle native 재생, `daily/bounded.rs`는 CPU admission/owned permit·2초 caller 수명, `daily/mod.rs`와`src/lib.rs`는typed module 진입점이다. `tests/unit/daily_replay.rs`·`daily_bounded.rs`는실제golden/변조/retained/정상거부와capacity·취소/timeout/panic/역순완료16개다. `scripts/check_auth_coverage.py`에Daily95%와누락거절을추가했다. README/ADR0050/설계/TS/추적/백로그/verification109 및107실제종료근거를갱신했다.

pm-skills correctness/changes·baseline6a2be6d의최종source/diff와검증을리뷰했다. 전체metadata권위·retained/current차이·exact command cache·정상거부·distinct definition 역순완료·actual permit취소/Timeout/panic/oversize에근거있는미해결범위내결함을확인하지못했다. API/DBwrite/공식시간·rank·UI가없음을확인했고기존규칙/DTO/schema/locks/SW/timeout/retry변경없음. ignoredreview/.tmp·원문·main보존. 이제최신develop/stage/head·commit/push/PR/최신CI를확인한다.109와상위19는아직OPEN이다.


## 첫 CI 실패와 종료·관측 수명 보정

PR110/head aa456fa의 첫 Repository38112810537 contribution-gate는 branch 접두사 feat/가 기존 feature/ 규칙과 달라 실패했다. 에이전트 생성 오류이며 gate를 완화하지 않고 feature/109-bounded-daily-replay로 고친다. GitHub는 PR head branch를 교체할 수 없어 같은 이슈의 새 PR로 대체하고110은 미병합 종료할 예정이다. #109/상위19는 아직 OPEN이다.

Product38112810532 실제PG18.6는60pass/1fail/ignored0/19.17초다. 기존 main owner-loss 검사가4초 내 프로세스 종료를 관측하지 못했다. stderr가 비활성이므로 원격 내부 원인은 미확정이다. ADR0051을 코드보다 먼저 작성했다. actual BoardPool SeedSource를 gate로 막은 뒤 fatal serve를 반환하는 검사에서 기존 runtime Drop은 gate 해제 전 반환하지 못했다: 0pass/1fail/Exit101/2.40초(.tmp/daily109-runtime-red.log). 동기 main이 runtime을 소유하고 fatal result에만 shutdown_background를 적용한 뒤2개0.01초PASS(.tmp/daily109-runtime-green.log). 정상 성공과 오류 전달을 보존한다. blocking 강제 중단을 주장하지 않으며 요청의 실제 CPU permit 수명은 그대로다. 실제PG17.4 61개7.59초/ignored0(.tmp/daily109-runtime-pg.log)도PASS했다. 기존4초/2초 기한과 retry는 바꾸지 않았다.

같은 첫CI web은211unit과 웹88.25%를 통과했지만 browser78pass/1flaky/1fail/16미실행/7.8분이다. 신규 이탈의 restart 뒤390px reload와1440px fresh에서 실제 latest200/body/fulfill 뒤 checking 화면이 지속됐다. 실패 trace의 다음 auth/bootstrap은 완료 응답 없이 남았다. artifact11691869507을 .tmp/daily109-ci-failure-artifact로 보존했고 원본 job 로그는 .tmp/daily109-ci-web-first.log다. 별도 offline 비교8개35.8초PASS를 본래 실패 해결 근거로 계산하지 않는다.

설치된 Playwright1.63 Page._onRoute의 times:1 마지막 handler 완료 뒤 비동기 interception 갱신과 후속 proof 경계를 검토했다. Chromium 내부 원인은 확정하지 않는다. ADR0049/설계14에 먼저 기록한 뒤 latest-result-observation.ts를 페이지별 WeakMap/한 persistent route·호출별 직렬 slot으로 바꿨다. 각 handler가 자기 slot을 캡처하고 실제 원본 POST/200/no-store/body를 전달한다. 일반 재탐색은 route.continue로 통과시킨다. 제품 TS/기한/retry/SW/SQL/규칙은 그대로다.

최초 로컬 반복 명령은2workers의 repeat 두 개가 공유 fixture 대기열에서 교차 매칭되어 본인/상대 match_id 불일치2fail/2미실행이었다(.tmp/daily109-observer-green.log). auth.spec의 serial 선언은 서로 다른 repeat worker까지 직렬화하지 않으므로 반복 명령에 workers1을 사용한다. 그 다음 초안은 관측하지 않은 기존 마지막 일반 재탐색을 거부해2fail/10미실행이었다(.tmp/daily109-observer-serial-green.log). unarmed 요청 continue로 계약을 보정했다. 후속 실제 반복/전체 검사/새 coverage/코드리뷰/최신 CI와 병합은 진행 중이다. 실패 결과를 완료 통과 수에 합산하지 않는다.

변경 코드와 목적: src/main.rs의 run/serve 분리 및 fatal-only runtime 종료, tests/unit/server_runtime.rs의 actual BoardPool blocked boundary와 정상 성공·오류 전달2개, latest-result-observation.ts의 페이지 수명/직렬 observer와 일반 요청 통과. ADR0051/0049·0050/설계00·14/검증109가 설계·추적·실패 근거를 보존한다. 외부키42·물리삭제25·미확정83은 여전히 별도다.


## CI 보정 뒤 실제 검증·추가 correctness 리뷰

페이지 수명 관측기 보정 뒤 실제 HTTPS·PG PC/mobile 이탈2개와 OS restart4개를 workers1/repeat2/retries0으로 실행해12개1.2분PASS했다(.tmp/daily109-observer-serial-final.log). 일반 재탐색과 후속 권한 proof·앱 표시·8언어/본인통계·DB 불변·실제 OS kill/restart 단언을 유지한다. Chromium 내부 원인 확정이나 미해결83 해결로 일반화하지 않는다.

진입점 변경 뒤 전용 새 .tmp/daily109-runtime-coverage-clean 빌드에서 coverage를 다시 측정했다. actualPG17.4 61개7.70초/ignored0, lib36개2.28초·main2개0.01초PASS. 최종 source 포함 Daily62/62=100%, 서버6968/7488=93.06%, Auth99.48/Match99.31/Lobby98.37/Result100% gate PASS(.tmp/daily109-runtime-coverage.json/log). 보정 전 보고서와 분모를 구분하고 exclusions를 추가하지 않았다.

pm-skills correctness/changes·baseline develop6a2be6d 추가 리뷰: async fatal 반환→runtime Drop→actual blocking worker→프로세스 종료 계약을 함께 봤다. gate가 닫힌 actual BoardPool을 통과한 fatal 반환의 deterministic Red가 있고, fatal-only background shutdown 뒤 같은 검사/실제 main PG owner-loss가PASS다. 정상 성공은 기존 graceful/Drop 분기를 유지하며 process 말미에서만 사용한다. caller timeout/permit 재사용과 혼동하지 않는다. 최초 원격 실패의 내부 원인은 로그만으로 단정하지 않는다.

관측기는 Page가 수명 권위이고 WeakMap의 ready가 설치를 한 번만 소유한다. 호출 slot을 handler 진입 때 캡처하므로 늦은 결과가 다음 호출을 덮지 않으며 같은 Page 동시 관측을 거절한다. armed 요청은 원본 strict POST/actual200/no-store/body/동일 fulfill, unarmed 요청은 original continue다. 설치/trigger/HTTP/JSON/fulfill 실패를 성공으로 바꾸지 않는다. 일반 재탐색 거부 초안은 실제 검사로 반증·수정했다. 현재 범위에서 근거 있는 추가 미해결 correctness 결함은 확인하지 못했다. 전체check·대체PR·최신CI·병합/109종료는 아직 진행 중이다.


보정 뒤 첫 전체check는 실제Exit1이었다(.tmp/daily109-corrected-check.stdout.log/stderr.log). 설치된 Playwright1.63의 route는 Promise<Disposable>을 반환하므로 초안의 ready:Promise<void> 타입과 맞지 않았다. 설치 완료를 await하기만 하는 경계에 ready:Promise<unknown>을 사용하고 pnpm typecheck 전체를 실행해Exit0을 확인했다(.tmp/daily109-observer-typecheck.log). TS 타입 선언만 바뀌었고12개 실제 browser 행동 및 Rust source/fresh coverage는 동일하다. 전체check는 마지막까지 다시 실행하며 이 중간 실패를PASS로 기록하지 않는다.


## 보정한 최종 소스의 전체 검사 완료

최종 scripts/check.ps1 실제 프로세스 Exit0(null거절): fmt/Clippy all-targets·333Rust·WASM/types/fixtures/tokens freshness·format/lint/typecheck·211unit37파일/web88.25%·production build·96browser3.3분재시도0·docs160/Mermaid34실패0 PASS. 로그는 .tmp/daily109-corrected-final-check.stdout.log/stderr.log입니다.

최종 diff/source를 baseline develop6a2be6d와 다시 검토했다. 서버 daily foundation과 fatal runtime 진입점, Page 수명 시험 관측기 및 설계/추적/실제 실패 기록만 포함한다. 일별 review/.tmp·원문·main·schema/DTO/규칙/의존성/lockfile을 보존했다. 새 PR을 제출하고 정확한 최신 head의 CI를 확인한 뒤에만 병합·109 종료·다음19하위를 시작한다. 현재 최신 원격 CI·새 PR·병합은 아직 미완료다.
