# #118 데일리 시간 권위 검증

FR09/10 · NFR01/02/05/07/08 · TS17/18/20/21/31/35/36. 선행116/PR117 병합·CLOSED를 확인하고 developc56832b에서 시작했다. [ADR0055](../adr/0055-bounded-daily-time-authority.md)를 코드 전에 작성하고 설계·추적표·백로그를 갱신했다. 제공된 design_layout11/12 원본을 실제로 확인했다.

## 코드와 행동

`daily/timing.rs`는 입력 ticket UUID·accepted metadata/rules·단조 시각으로 동작하는 순수 정책이다. 시작 TTL=countdown+duration, 권장 heartbeat15초/실패 경계45초, 연속 순번·마지막 duplicate의 원래 ack, 영구 강등과 최소 native replay elapsed를 검사한다. 첫 성공의 account/attempt/metadata/digest/서버 시간은 private `VerifiedDailyTime`이며 동일 재전송은 원래값, 다른 digest는 conflict다. 클라이언트 elapsed를 공식 시간으로 복사하지 않는다.

`daily/tickets.rs`는 Clock port·계정당1개/전체1..20000·같은 AuthorityRegistry의 짧은 memory commit과 실제 SessionInvalidator barrier를 조립한다. 만료/철회/교체/clock 실패 시 entry는 두 lock 밖에서 drop한다. 인증 lease 없이 만료 자원만 회수하는 `reap()`를 제공하며 후속 HTTP는 새 generation/read/bind 전에 호출해야 한다. raw replay/닉네임/추가PII를 캐시하지 않고 session hash는 기존 authority에만 둔다. 새 dependency·migration·공개 DTO/API·main 활성화는 없다.

## 실제 Red·리뷰·Green

- 시작 재전송: 미구현 Unavailable로0pass/1fail → 원래 ticket/TTL 유지.
- 마지막 lease 수명: 다른 caller가 끝난 만료 entry 정리 시 `ConnectionAuthority::Drop`이 이미 잠긴 authority에 재진입,0/1/1.01초(channel Timeout) → retired 목록을 양 lock 해제 후 drop. 만료·교체·clock 실패·철회와 request 사이의 lease 보존/역순 late finish를 검사했다.
- 시계 역행: 순수 정책의 finished→역행→증가에서 이전 proof가 재반환되어0/1/0.17초(Some11000/None) → 역행 시 cached finished를 버리고 영구 강등. adapter도 registry 전체를 fail closed로 고정한다.
- 만료 admission: authority와 ticket의 용량이 같을 때 만료 lease가 새 계정 bind를 막아0/1/0.00초(Capacity) → 인증 bind 전에 호출할 reap으로 만료 자원을 회수한다. 미만료 자원은 유지하고 reap에도 역행/poison/drop 정책을 적용한다.

correctness/changes baselinec56832b로 producer/consumer·수명·실제 authority와 accepted snapshot·동시 시작/완료·계정/hash 철회 barrier/late bind·원래 attempt/digest·None/실패·정확 경계를 순차 리뷰했다. 최초 fixture28pass/1fail은 변경한 rules의 hash를 재생성하지 않아 InvalidDefinition이었고, `RulesSnapshot::from_rules`로 유효 accepted snapshot을 만들어 고쳤다. 제품 검증을 완화하지 않았다.

최종 관련 Daily 도메인·native·worker44개/3.23초 통과. 실제PG17.4 회귀85개/12.19초 통과. 이는 기존 production HTTP/rights/PG 기능 회귀이며 아직 공개하지 않은 시간 API의 검증으로 계산하지 않는다. fresh coverage와 전체 검사 결과는 아래에 기록하며 원격 CI/PR/병합은 아직 완료 전이다.

최종 소스를 새 target에서 instrument한 실제PG85개/12.17초 포함 전체 검사가 통과했다. timing128/128100%·tickets203/203100%, Daily6파일684/68799.56%·Privacy100%·전체8640/923493.57%다. timing/tickets를 각각 보고서에서 제거한 negative gate는 실제Exit1 missing domain이다. 기존 도메인95/전체80·entry point 포함 기준을 유지했다. 이전 중간 측정은 소스 보완 전이므로 최종 근거로 사용하지 않는다.

## 제한과 다음 순서

재시작은 빈 registry여서 이전 ticket은 no-proof다. futurePG는 원래 first receipt를 불변으로 유지해야 한다. 공개 seed 사전 계산·봇·인간 풀이·전세계 latency 공정성을 이 proof로 증명하지 못한다. 서버 관측시간에는 네트워크 지연이 포함된다.

후속 이슈에서 본인 export의 nullable 서버 시간 필드를 먼저 추가한 뒤 영속 시간/인증 start·heartbeat·submit API를 활성화한다. 두 순위·8언어 명시 제출/대기 복구·상위19는 미완료다. 실제OAuth42·물리 삭제/백업25·하드웨어26·사람27·기존 미해결SW83은 별도다. 일별 리뷰는 Git에서 제외한 로컬 파일에 보존한다.


## 첫 전체 검사 실패 관측

첫 full은 Rust/web unit까지 통과하고 browser61pass/1fail/38미실행(2.5분)에서 Exit1이었다. 기존 reconnect reload1440px 시험의 두 번째 login 페이지가 `/assets/NavLink-EJTuc34W.js`를 `net::ERR_NO_BUFFER_SPACE`로 불러오지 못했다. trace에 해당 network status-1/console error·auth bootstrap 이전·Continue with Google 대기가 있고30초 test timeout 뒤 context dispose 오류가 이어졌다. 이 두 번째 오류를 최초 원인으로 삼지 않는다. 실패 trace/DOM/log를 `.tmp/daily118-first-full-failure`와 원래 full 로그에 보존했다. #118 시간 모듈은 main에서 활성화하지 않았으며 기존 웹/fixture 제품 소스를 바꾸지 않았다. 브라우저/OS 원인이나 기존83 SW 실패와 같은 원인인지 미확정이다. 실패 이후 OS read-only 관측은 TimeWait37/동적TCP포트16384로, 이를 실패 순간의 포트 고갈 증거로 사용하지 않는다.

소스·timeout·retry 변경 없이 해당 기존 재접속 PC/mobile 및 전체 검사를 새 output으로 다시 실행한다. 성공하더라도 최초 실패와 미확정 제한을 함께 기록한다.


## 실제 전체 검사

scripts/check.ps1 실제Exit0: Rust380·web220/38files/Lines88.83%·browser100/3.5m/재시도0, fmt/Clippy/WASM/types/fixtures/tokens/format/lint/typecheck/build·문서168/추적36/Mermaid34실패0 통과. 측정 이후 Rust source/test 변경은 없고 두 신규 domain 파일을 포함한 gate95/80을 유지했다. 원격 CI/PR·exact-head 병합/118 CLOSED는 다음 절차다.
