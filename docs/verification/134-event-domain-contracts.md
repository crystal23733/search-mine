# #134 목적·보존·NSM/D7 도메인 계약

[이슈134](https://github.com/crystal23733/search-mine/issues/134) · [ADR0063](../adr/0063-event-domain-contracts.md) · M3/상위20.

선행19 CLOSED·PR133/develop0d25f9f 병합과 clean develop을 실제 확인한 뒤 feature/134-event-domain-contracts로 분기했다. 제품 코드 전에 ADR0063/TS29·목적·권위·보존·지표의 완결성과 추적을 정의했다. 저장/수집 이전 Rust 계약이며 PostgreSQL event 수집·물리 retention·실제 이용 NSM/D7·CMP는 후속이다. 이 foundation으로 #20 전체를 종료하지 않는다.

## 변경과 실제 행동 검사

| 경계 | 구현·검증 |
|---|---|
| client→정규화 | decoder만 ClientDraft 생성; 최대1024 UTF-8 byte, v1/canonical nonnil UUID/8locale/닫힌9종·날짜/숫자 한계. 추가 민감필드·server 권위·동의 주장은 거절 |
| 동의→입장 | analytics/advertising 별도, 사건·수집 시각을 덮는 유효 grant 및 같은 session subject 필요. unknown/denied/failed/만료/발생 뒤 동의/nil revision/역행 거절 |
| 재전송→동일성 | 원래 event ID·subject·시각·locale·목적·전체 내용 불변. 동일/충돌/다른ID 분리. 원래시각30일과 UTC calendar13개월 정확 경계·월말·윤년·DST/overflow |
| 서버 fact→NSM | 별도 trusted Rust match_end/daily_verified. NSM은 clear/timeout, bot 포함, account+match 중복 제거, 사람2명 참여 횟수 구분, 포기/abort/daily/client 제외 |
| 첫 날짜→D7 | 명시 authoritative 첫 정상 날짜, 정확+7 UTC일. 보존된 첫 관측을 평생 첫 날짜로 추정하지 않음. 미성숙 null·분모0·삭제 후 입력 제거·불완전/오염/10000상한 |

행동 테스트는 [event_model](../../crates/server/tests/unit/event_model.rs), [event_policy](../../crates/server/tests/unit/event_policy.rs), [event_metrics](../../crates/server/tests/unit/event_metrics.rs), [public decoder](../../crates/server/tests/events.rs)에 있다. 필수 server source에는 정답/seed·IP·토큰·OAuth·nickname·임의 payload를 넣지 않는다. client 광고 claim을 실제 SDK 노출이나 정산 증명으로 해석하지 않는다.

실행 Red는 stub decoder에서 유효 입력을 거절해 0pass/1fail(exit101)이었다. 리뷰 중 같은 grant를 다른/누락 session에서 재사용하는 테스트도 실제 assertion Red0pass/1fail 뒤 subject 일치 검사로 수정했다. 최종 `cargo test --locked -p liar-server --lib events::` 15pass, 별도 `--test events` 1pass다. 직접 Deserialize로 검증을 우회하는 경로는 private WireClientDraft만 역직렬화하도록 닫았다. full 첫 실행의 테스트 clone clippy 경고2곳은 from_ref로 수정했다.

## 실제 계측

로컬 PostgreSQL17.4 격리 DB에서 기존 회귀101pass/0fail/15.69초. 별도 격리 DB의 fresh `cargo llvm-cov --locked -p liar-server --tests … -- --include-ignored`는411pass, 그중 실제 DB101pass/18.98초다. 커버리지는 다음과 같다.

| 대상 | 실제 줄 커버리지 | 게이트 |
|---|---|---|
| Event model | 95/95, 100% | events 합계95% 이상 |
| Event policy | 88/89, 98.88% | events 합계95% 이상 |
| Event metrics | 152/152, 100% | events 합계95% 이상 |
| Event 합계 | 335/336, 99.70% | 통과 |
| Daily | 877/893, 98.21% | 95% 통과 |
| 서버 전체·main/migrate 포함 | 9425/10036, 93.91% | 80% 통과 |

Auth99.48%, Match99.31%, Lobby98.37%, Result/Privacy100%도 실제 통과했다. `scripts/check_auth_coverage.py`에 events3개 파일의 필수 존재/95% 조건을 추가했다. 실제 보고서에서 model/policy/metrics를 각각 제거한3개 입력과 event covered를0으로 낮춘 입력 모두 exit1을 확인해 누락·미달을 성공으로 처리하지 않음을 검증했다. DB 없는 기본 cargo test의 ignored 검사는 통과로 계산하지 않았다.

DB를 연결한 `scripts/check.ps1` 최종 exit0: fmt/clippy, Rust workspace415pass(실DB101ignored는 별도 실행), WASM·generated types/fixtures/tokens·format/lint/typecheck/build, 웹44files/265pass·줄90.83%, 실제 Chromium113pass/5.4분을 확인했다. 문서184/제품21/설계18/시나리오36, Mermaid34/failure0, diff --check도 통과했다. 원격CI/정확head 병합·CLOSED 결과는 해당 develop PR·이슈와 일별 로컬 리뷰의 실제 상태에 기록한다.

## 리뷰와 미확인 범위

정확성 changes 리뷰의 기준은 develop0d25f9f다. decoder→event 권위, consent→session identity·기간, retry→immutable equality, fact→NSM/D7의 정상 predicate·중복·표본·첫 날짜·null, UTC/byte/입력 상한을 검토했다. 동의 세션 연결과 decoder 생성 경계를 수정한 뒤 검토 범위에서 추가로 뒷받침된 결함은 없었다. ID 없는 counts/기간 출력과 명시 incomplete 실패는 지표 오해를 막는다.

snapshot은 저장 권한 proof가 아니다. 현재 consent revision/계정 상태를 같은 PG transaction에서 검증하는 저장, committed producer, 본인 export/삭제, first-normal anchor·집계 저장/retention·운영 조회는 다음 필수 #20 하위다. #21 연결 전 비필수 수집은 켜지 않는다. 기존 외부42·운영25/26·사람27·미확정83과 목표 성능·법적 검수는 이 결과로 대체하지 않는다.
