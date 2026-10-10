# 0044. 온라인 결과 저장의 단일 소유 연결

상태: 채택 · M3 #18/#97 · FR14/16, NFR01/02/05/07/08 · TS15/16/20/21/31/35/36. 선행95/PR96 developfd29c11 병합·종료 확인.

## 의무와 결정

재시작 복구가 살아 있는 다른 서버의 매치를 중단하거나 이전 서버의 늦은 write가 복구 결과를 덮어쓰지 않아야 한다. 현재 종료 결과만 저장하는 구조는 이 조건과 active journal을 아직 제공하지 않는다. 먼저 모든 온라인 결과 write와 소유 session advisory lock을 같은 전용 연결에 묶는다. pool 연결의 lock과 다른 연결의 write를 조합하지 않는다.

pool.acquire().detach()로 얻은 raw PgConnection 하나가 lock·save·heartbeat를 직렬 실행한다. 자동 재접속·pool 반환이 없고 connection loss 뒤 다른 연결로 저장을 이어가지 않는다. lock key는 고정 앱 namespace 0x4c53575200000000과 online_match_results relation OID의 합이다. 같은 DB/결과 relation은 한 소유자만 허용하고 테스트의 별도 schema는 분리한다. try-lock 실패·연결/기한 실패는 초기화를 거절한다. advisory lock은 협력하는 앱의 규약이며 DB 권한이나 외부 unfenced SQL을 차단하는 장치라고 주장하지 않는다.

ResultRepository port/FinishedMatch/known JSON을 유지한다. 기존 PgResultRepository의 순수 입력 검증과 transaction을 공통 raw connection 함수로 분리한다. 입력 검증은 DB acquire 전에 유지한다. bounded worker는 대기16개·동시 SQL1개, save는 큐 대기를 포함해2초 deadline·heartbeat1초/전체2초다. try_send 포화는 Unavailable이며 소유자를 종료하지 않는다. 시작 전 취소/만료 작업은 SQL을 하지 않는다. 시작한 SQL의 caller 취소는 commit 여부를 조작하지 않으며 UUID idempotency로 ACK 손실을 처리한다. SQL 시작 뒤 기한 초과/Unavailable/연결 종료는 owner를 영구 실패시키고 연결을 폐기한다. Malformed/immutable 충돌은 owner를 종료하지 않는다. worker panic/상태 채널 종료도 실패다. 마지막 저장 handle drop은 연결 종료로 lock을 해제한다. 기존 actor3회/2초 retry와 게임 기한은 바꾸지 않는다.

main과 실제 HTTPS fixture에 소유 연결을 구성한다. main의 HTTP future는 owner failure를 함께 감시하고 오류로 종료하며 Tokio runtime 종료가 기존 WS/actor를 멈춘다. ready는 DB 검사 전후 owner 상태를 확인한다. 정상 ctrl-c는 기존 graceful shutdown을 유지한다. auth 미설정 local-only 실행은 owner를 만들지 않는다. 전용1연결은 기존 auth pool 최대5연결 외의 추가 연결이며 실기기 용량은26에서 측정한다.

실패 관측은 save 오류 또는 heartbeat의 최대1초 대기/최대2초 IO 예산에 의존한다. 관측 전의 짧은 WS 동작을 즉시 중단한다고 보장하지 않으며 이 예산을 OS 스케줄링까지 포함한 성능 실측으로 보고하지 않는다. 이전 소유자의 durable write 차단은 애플리케이션 watch와 별도로 같은 DB 세션/lock의 수명으로 보장한다.

이후 journal/admission/finish/recovery write도 반드시 같은 owner worker를 사용한다. 기존 unfenced 앱을 종료한 뒤 새 단일 인스턴스로 교체한다. rolling/hot failover를 지원한다고 주장하지 않는다. 현재 작업에는 active journal·startup abort·실제 OS kill/restart 결과 복구·새로고침 결과 발견이 포함되지 않는다. 개인정보/secret/규칙/화면·외부 OAuth 권한은 바꾸지 않는다.

인증·계정 삭제와 FK cascade·본인 결과 조회는 기존 pool을 사용한다. 소유 연결의 범위는 온라인 입장/종료/복구 write이며 개인정보 삭제를 막지 않는다. account key-share/삭제 비복원 계약은 그대로다. PgResultRepository의 기존 직접 구성은 독립 adapter 검증에 남지만 main/HTTPS fixture의 정상 저장에는 사용하지 않는다. loopback unknown-result fixture의 별도 SQL은 계약 주입이며 소유 연결을 통한 실제 장애 복구로 보고하지 않는다.

## 검증

실제 main 두 프로세스/같은 DB에서 두번째 거절을 Red로 확인한다. 실제 PG 단일 owner·정상 drop/재획득·backend 강제 종료/오래된 save 불가·SQL lock 지연/rollback·caller 취소/중복·삭제 비복원·큐 포화/시작 전 취소·Malformed 이후 정상 저장을 검증한다. 실제 main owner loss 뒤 비정상 종료/HTTP 중단과 ready fail-closed를 확인하며 backend 종료를 실제 OS kill/restart 복구와 혼동하지 않는다. 관련 전체 gates와 domain95/전체80을 실측하고 최신 CI/head·코드 리뷰 뒤 병합한다.

근거: [PostgreSQL session advisory lock](https://www.postgresql.org/docs/18/explicit-locking.html#ADVISORY-LOCKS), 설치된 SQLx0.9 공개 PoolConnection.detach/Connection 계약. session 종료는 lock을 해제하며 transaction rollback은 session lock을 해제하지 않는다. 후속 journal의 보존 기준 시각과 비동기 입장 보상은 별도 ADR에서 정한다.
