# #126 서버 확정 데일리 규칙 초기화

[이슈126](https://github.com/crystal23733/search-mine/issues/126) · [ADR0059](../adr/0059-accepted-daily-core-initialization.md) · M3/상위19.

선행124/PR125 develop21f6506 병합·CLOSED를 확인한 뒤 설계/추적/백로그부터 갱신했다. Native with_rules/from_replay_with_rules·16KiB closed JSON·WASM withRules·생성 RulesSnapshot의 선택적 worker 전달을 구현했다. 기존 두 인자 init는 bundled이며 잘못된 snapshot은 fallback하지 않는다. 생성 bindings는 wasm:generate로 갱신했다. Rust verify/derive/core만 규칙을 판정한다.

## 실제 검증

scripts/check.ps1 실제 exit0: Rust399개·웹221개/38파일/line88.85%·브라우저106개/3.9m/재시도0, WASM/타입/fixture/format/lint/build·문서176개/Mermaid34개 실패0. 최종 JSON 계약 테스트까지 포함한다. Native 데일리93/94=98.94%, WASM crate host290/341=85.04% (FFI lib.rs 포함).

실제 worker 행동 Red는 기본3000ms/hash를 반환하여1실패/exit1이었다. 컴파일 누락을 행동 Red로 세지 않았다. Rust core golden은 countdown5000/다른 hash·seed/70개 public 입력의 완료다. 기본 fixture의 기존 v/cases/daily는 deep equality로 변경 없음을 확인했다. 초점 실제WASM/worker PC·mobile4개/8.0초, 최종 Native6개/0.85초를 통과했다. strict16KiB UTF-8/정확 상한·중복/추가 필드·손상 hash/버전·accepted replay 복원·더 작은 명령 상한/기존 wire8192·거절 뒤 not_started/정상 재초기화를 확인했다.

최초 host coverage76.25%(260/341)는 lib.rs JSON 경계 미실행을 포함한 실제 수치다. 보고서/로그를 보존한 뒤 공개 JSON API와 golden의 행동 계약 테스트를 추가해 위85.04%를 얻었다. source lib.rs를 커버리지에서 제외하지 않았다. 브라우저 fixture 실행 중 final Rust 검사를 병행한 첫 시도는 Windows auth_fixture.exe 잠금/LNK1104/exit101이었다. 실패 로그를 보존하고 브라우저 정상 종료 뒤 재실행했다. 테스트 timeout·retry는 완화하지 않았다.

최초 전체 브라우저는103통과/1실패/2미실행/3.8분이었다. 기존 저장소 거부 테스트의 preact.js 로딩이 ERR_NO_BUFFER_SPACE로 실패하여 빈 app 상태였고 gridcell256 조건에 도달하지 못했다. 로그/trace를 보존했으며 오류의 근본 원인을 확정했다고 주장하지 않는다. 병행 계측/빌드가 종료된 뒤 새 DB에서 같은 timeout·retries0으로 전체 재실행을 통과했다. 기존 SW83과 이 관측을 같은 원인으로 단정하지 않는다.

서버/core/protocol wire/DB schema는 변경하지 않았다. 로컬 전체 HTTPS는 새 전용 DB liar_daily126b에 기존 migration을 적용해 실행하여 이전 테스트의 미래 기록을 섞지 않았다. 원격 제품 CI는 실DB101개와 core/server95/80 게이트도 다시 실행한다. OAuth42·운영25/26·사람27·미확정83은 미완료다.

## 코드 리뷰와 후속

pm-ai-shipping:code-review correctness/changes/baseline21f6506를 순차 적용했다. 입력 크기/JSON/규칙 hash·버전, core 단일 원천, metadata 결합 복원, worker의 성공 후 mode 전환, 기존 fixture/두 인자 init·wire replay 상한을 검토했다. 검토 범위에서 추가로 근거가 뒷받침된 결함은 없다. 독립 보안 감사·실기기/인간/RTT 검증은 수행하지 않았다.

아래 실제 원격CI·정확head 병합·CLOSED 기록으로 종료했다. 공식 start/heartbeat/순위 UI·session-bound pending·실제 HTTPS UI와 상위19 전체는 후속이다. snapshot 전달은 인증/시간/공식 제출 권위를 부여하지 않는다.


## 실제 CI·병합 종료

Product CI38251715502 성공: Rust399·PG18.6 101/40.23초·계측101/35.36초, Daily877/893=98.21%·서버9090/9700=93.71%·core95% gate·웹221·브라우저106첫통과/4.5분/flaky0. 최신8checks/3runs 성공과 정확03d355de643f7e91a8a6d68437b88c2d04b22f38·clean tree를 확인했다. [PR127](https://github.com/crystal23733/search-mine/pull/127) MERGED2026-10-12T13:05:06Z/developa5dbdecb4bc83d92c36b677230e003d9160be631, #126 CLOSED13:05:07Z 별도 확인. 후속128과 공식UI/상위19는 별도다.
