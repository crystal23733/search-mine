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
