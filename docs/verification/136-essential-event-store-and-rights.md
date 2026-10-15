# #136 필수 이벤트 저장과 본인 권리

[이슈136](https://github.com/crystal23733/search-mine/issues/136) · [ADR0064](../adr/0064-essential-event-store-and-rights.md) · 상위20/M3.

선행134 CLOSED·PR135/develop0155012 병합과 clean 최신 develop을 실제 확인하고 feature/136-essential-event-store로 분기했다. 제품 코드 전에 ADR0064/TS29·권위·dedup·30일·본인 권리·default-deny를 정의했다. 자동 수집/물리 retention/집계/CMP는 후속이며 이 하위로 #20 전체를 종료하지 않는다.

## 구현·시나리오

| 경계 | 확인할 결과 |
|---|---|
| server source→PG | 실제 committed 본인 인간 match/daily ID에서만 fact·원래시각을 유도. client draft/임의 ProductEvent 저장 API 없음 |
| identity→retry | event ID unique·본인 source unique·불변 trigger. 동일/충돌·새ID의 같은source 경합에서 기존 accepted ID/received_at 유지 |
| account/time→write | 같은 transaction의 account 잠금·source share lock, 정확30일·원본 게임 보존·future/rollback·2초IO. 삭제/만료 뒤 복원 없음 |
| 목적·필드→DB | essential CHECK·두 fact의 XOR/필수 nullable CHECK·FK cascade. JSON payload·seed/보드·IP/토큰/OAuth/nickname 미보관. optional API 부재 |
| own export→웹 | 최근 재인증/현재 revision·Origin/CSRF/전후 proof·2초·50개/페이지. Rust→TS event DTO/세 스트림·2048페이지/32MiB/100ms pacing/취소·명시 파일 다운로드 |

[PG 행동 테스트](../../crates/server/tests/postgres/events.rs)는 source 미확정/외부계정·중복 seat, 동시 event ID/계정 충돌, 원본 재시도, 원래30일과 삭제·실제 잠금/IO timeout, 불변/목적/민감 CHECK, 51개 본인 페이지·철회/reauth, plain/sealed daily와 손상된 시간 거절을 확인한다. [privacy pure 검증](../../crates/server/tests/unit/privacy_model.rs), [웹](../../apps/web/src/services/auth/export.test.ts), [HTTPS8언어 다운로드](../../tests/e2e/helpers/personal-export.ts)를 연결한다. `__fixture/events/{id}`는 loopback browser example의 명시 제어이며 production 수집 API나 자동 producer가 아니다.

## 실제 Red→Green과 계측

실제 PG에 본인 match를 확정한 뒤 stub capture가 ErrUnavailable을 반환하는 assertion Red0pass/1fail(exit101)을 확인했다. 웹은 새 event 스트림 기대가 기존2개 호출과 달라 assertion1fail/9skip이었다. 준비 단계의 sandbox Vitest EPERM과 sqlx chrono Encode/Decode 빌드 실패는 행동 Red로 계산하지 않았다. 기존 date text bound/to_char 패턴으로 해결했고 의존성을 추가하지 않았다.

정확성 changes 리뷰 기준0155012에서 기존 Daily export가 거절하는 replay3200ms보다 짧은 stored verified_elapsed_ms=1을 새 event가 VerifiedTime으로 수용하는 결함을 실제 PG Red0pass/1fail로 확인했다. 기존 Rust publication/export_receipt 검증을 재사용해 수정했고 유효 sealed time도 확인했다. 최종 새 PG10pass/3.10초, privacy event unit1pass, 웹export12pass/3.92초, typecheck·all-targets clippy 통과.

로컬 PostgreSQL17.4 격리 DB 전체111pass/0fail/17.91초. 별도 fresh llvm-cov는422pass(실DB111/19.94초)이며 다음 게이트를 통과했다.

| 대상 | 실제 줄 커버리지 | 기준 |
|---|---|---|
| Privacy domain | 182/182, 100% | 95% |
| Event domain | 335/336, 99.70% | 95% |
| Daily domain | 877/893, 98.21% | 95% |
| 서버 전체·main/migrate 포함 | 9713/10338, 93.95% | 80% |

Auth99.48%·Match99.31%·Lobby98.37%·Result100%도 실제 통과했다. DB 없는 기본 cargo test의 ignored 검사를 통과로 계산하지 않는다. 최종 scripts/check.ps1 exit0: fmt/clippy·Rust416pass(111ignored DB는 별도 실행)·WASM/types/fixtures/tokens/format/lint/typecheck/build, 웹44files/267pass·줄90.91%, 실제Chromium113pass/5.5분·retry marker0, 문서186/제품21/설계18/시나리오36·Mermaid34/failure0·diff --check 통과. PC1440/mobile390에서8언어 실제 HTTPS 다운로드가 own online/event2건·accepted ID 유지·secret 제외를 확인했다. 원격CI·정확head 병합/CLOSED는 해당 develop PR/이슈와 일별 로컬 리뷰의 실제 상태에 기록한다.

최종 정확성 검토는 source→권위/시간·account/source lock→삭제 경합, ID/source unique→accepted ID·immutable equality, export→현재 own proof·페이지/바이트·태그/시간·취소를 양쪽에서 대조했다. Daily receipt 검증 수정 뒤 검토 범위에서 추가로 뒷받침된 결함은 없었다. 선택 이벤트와 자동 runtime/물리 보존·집계는 검토 완료 기능으로 계산하지 않는다.

## 남은 범위

읽기/재시도는 원래30일 만료를 적용하지만 물리 cleanup worker·자동 runtime producer·first-normal anchor/NSM/D7 집계 저장/운영 조회는 다음 필수 #20이다. analytics/advertising는 아직 저장할 수 없으며 현재 consent subject/revision의 동일 transaction 검증은 #21 실제 consent adapter를 연결할 때 필수다. 법적 검수·목표 성능·실OAuth42·운영25/26·사람27·미확정83의 완료를 주장하지 않는다.
