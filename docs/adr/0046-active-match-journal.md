# 0046. 영속 active journal의 저장·복구 경계

상태: 채택 · M3 #18/#101 · FR14/16, NFR01/02/05/07/08 · TS15/16/20/21/31/35/36. 선행99/PR100 developbd1f48fe 병합·종료. 기존 승인된 장애 abort/최소 개인정보/단일 저장 소유권을 구현하는 저장 foundation이며 공개 입장 활성화는 후속이다.

## 범위와 구성

이 storage 작업은 최소 active schema와 typed journal runtime의 register/discard/strict final/startup abort를 실제 PG로 검증한다. 아직 lobby/main/HTTPS 정상 구성은 기존 PgResultRuntime이다. 후속 admission 통합에서 양쪽을 함께 strict 구성으로 전환한다. foundation을 공개 사용자 프로세스 복구 완료라고 보고하지 않는다. 직접 DB 호출이나 제품 bool bypass를 추가하지 않는다.

PgJournalRuntime은 private PgResultRuntime client/동일 bounded worker를 공유하는 별도 타입이다. claim_connection을 공통화해 기존 OWNER_NAMESPACE+online_match_results OID를 유지한다. 내부 mode enum은 공개 bypass가 아니며 journal 타입에서만 strict ResultRepository/AdmissionJournal port를 제공한다. 소유 UUID는 claim마다 새로 만든다. claim이 같은 raw lock 획득→startup 복구를 완료한 뒤에만 watch(true)/worker/typed handle을 반환한다. 실패/timeout은 raw 연결 폐기, healthy 반환 없음. 기존 16queue/2초/heartbeat/단조 실패를 유지한다.

## 스키마·입장

active parent: UUID/hash/owner UUID/admitted UTC 정수초. child: match FK cascade/seat0~1/account FK cascade/is_bot. seed/board/명령/최종 통계/session/provider/nickname 없음. id/owner nonnil, hash64lowerhex, UTC epoch0..253402300799-604800 정수초 CHECK (seed 만료 시각 상한 포함). participant account bot 관계·seat PK·동일 human 중복 금지. cross-row 두 참여자/최소1human 계약은 typed register가 검증한다.

AdmissionJournal::register(ActiveMatch{id,rules_hash,players})는 journal runtime의 AuthClock 제출 시각을 admitted_at으로 사용한다. caller가 owner/time을 지정하지 않는다. 모든 human account를 FOR KEY SHARE로 확인하며 missing은 거절한다. 같은 UUID의 기존 final에는 등록하지 않는다. active duplicate는 같은 owner/hash/원래 두 participant와 정확히 일치할 때만 Duplicate; 삭제로 자식 누락이면 재삽입하지 않고 거절한다. parent/두 child는 하나의 transaction. 서로 다른 owner/다른 hash/account/seat 재사용은 거절한다.

discard(id)는 같은 owner의 active만 transaction으로 삭제한다. absent는 idempotent, 다른 owner는 거절, final은 삭제하지 않는다. 시작 전 취소는 무효과. register commit 뒤 reply receiver가 닫혔다면 같은 raw worker에서 bounded discard 또는 영구 owner failure로 넘겨 고아를 방치하지 않는다. receipt를 받은 뒤 caller 취소의 확정/보상은 admission 통합의 별도 의무다.

register future의 queued/started/cancelled/observed 원자 상태와 worker AbortHandle로 ACK 전달 직전·직후 취소도 덮는다. queued 취소는 해당 명령만 skip하고 SQL을 시작하지 않는다. started 이후 관측 전 취소는 commit 여부를 추정하거나 Duplicate active를 지우지 않고 owner를 종료하여 startup 복구에 넘긴다. 정상 취소는 후속 admission이 register 완료를 관측한 뒤 명시 discard해야 한다. 이 보수적 취소 경계의 owner 종료를 무중단 취소 보장으로 표현하지 않는다. watch sender는 worker만 소유하며 client guard가 failure 관측을 숨기지 않는다.

## strict final

일반 FinishedMatch 순수 검증을 유지한다. 같은 owner/hash의 active를 FOR UPDATE하고 살아남은 participant 행만 memory seat/account/is_bot과 비교한다. 계정 삭제로0/1행인 것은 허용하고 메모리 참가자를 다시 삽입하지 않는다. 결과 parent/살아남은 child/active 삭제는 같은 transaction. 부모 retention_started_at=admitted_at, recorded_at=실제 now; now<admitted는 fail-closed. seed는 admission 기준 최대604800초만 허용하고 이미 지났으면 NULL. 정상 legacy writer의 기존7 calendar days 계산과 이 경계를 섞어 보고하지 않는다.

이미 final인 UUID는 기존 immutable hash/seed(삭제 seed 예외)/elapsed/reason 검증 후 Duplicate, 시각/children/anchor 비복원. final과 active가 동시에 존재하는 비정상 상태는 추정해 합치지 않고 fail-closed. active도 final도 없으면 거절한다. final participant 통계는 기존 validate_result 범위와 enum 변환만 사용한다.

## startup abort

모든 이전 owner의 active가 대상이고 현재 actor 재개/seed 복구는 하지 않는다. 최대10000개를 사전 bounded 확인하고 batch64를 처리한다. 소유 잠금 획득2초와 별도로 사전 조회 포함 복구 전체30초·각 batch 전체2초 기한을 둔다. 초과/역행 UTC/오염/DB 실패는 healthy 반환 없이 거절한다. 부분 batch commit은 재실행 가능하며 각 매치의 final+active 삭제는 원자적이다. 이미 처리된 행은 재삽입하지 않는다. 기한은 실제 하드웨어 처리 용량/성능 보장이 아니다.

결과 server_failure/abort/completed:false/end_elapsed_ms:null/own:null, secret_seed:null, anchor=알려진 admitted_at/recorded_at=실제 now. 삭제 후 살아남은 child만 옮기며0행도 허용. final과 active 동시 존재는 위 오염 계약으로 거절한다. admitted 이후90일 경과 결과는 즉시 reader에서 숨긴다. seed_expires_at은 admitted+604800초이고 새 복구 시각으로 연장하지 않는다. 신버전 strict 활성화 뒤 legacy app로 롤백하지 않고 forward fix한다.

## 검증

schema/typed storage 실제 Red→Green; 최소 schema/제약·atomic partial rollback·same-owner idempotency/conflicts·FK삭제/입장 retry 거절·strict final/삭제 비복원·정확 seed7일·보존 anchor·owner loss/queue/ack 취소·새 claim unknown abort/이미 final 불변/0participant/복구 중 오류 rollback/overlimit/UTC역행을 실제 DB로 확인한다. 기존 owner/runtime/browser 회귀와 domain95/전체80/docs/Mermaid/pm-code-review/CI가 필요하다.

실제 OS kill/restart와 인증된 기존 WS·async lobby admission의 cancel/logout/expiry/lease 재검증/중복 공개·신규 match ID 발견은 다음 통합에서 검증한다. 공개 storage에 match ID를 저장하지 않는다. 상위18/83/외부42/26/27은 유지한다.


## 운영과 한계

admitted_at은 완료 시각이 아니라 durable 입장을 요청한 실제 UTC이며 countdown은 후속 admission ACK 뒤 시작한다. register 전에 공개 matched를 내보내지 않는다. register가 commit된 뒤 공개 전 프로세스가 종료되면 재시작에서 unknown abort가 생길 수 있으며 completed:false다. 실제 미확인 플레이/종료 수치를 만들어내지 않는다. 90일은 프로젝트 정책이고 법정 의무라고 주장하지 않는다. 물리삭제/백업25는 같은 anchor와 journal을 정리해야 하며 이번에 완료됐다고 주장하지 않는다.

seed 경계는 저장 시점의 나이 검사와 seed_expires_at 메타데이터를 뜻한다. 이미 저장된 seed와 백업의 실제 물리 제거는25에서 검증하며 이번 쓰기/복구 검사로 대체하지 않는다.

부분 batch 완료 뒤 실패하면 readiness 없이 종료하며 재시도에서 남은 active만 처리한다. 만료된 admission도 unknown 결과로 정리하되 본인 reader가90일 보존 기준으로 즉시 숨긴다. 계정 삭제의 FK cascade는 소유 worker와 별도 pool에 남으며 복구가 삭제된 참여자를 되살리지 않는다. 일반 main의 기존 정상 저장·기존 known JSON/seed 계산과 migration checksum을 유지한다. 새 journal 타입을 사용하지 않는 현재 공개 실행의 OS 재시작 복구 완료를 주장하지 않는다.

근거: [ADR0044 단일 소유 연결](0044-online-storage-owner.md), [ADR0045 보존 기준](0045-result-retention-anchor.md), [PostgreSQL session advisory lock](https://www.postgresql.org/docs/18/explicit-locking.html#ADVISORY-LOCKS). public API/TS/UI/게임 규칙·OAuth 필드는 변경하지 않는다.
