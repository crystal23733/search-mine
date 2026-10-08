# ADR0023 · 권위 매치 actor와 인증된 WebSocket

2026-10-08 · 승인된 PRD 구현 · #16/M3 · FR02~05/15 · TS14/20/21/22 · 선행 #46 PR50 병합/종료. 기존 RuleEngine과 공개 관측 정책을 재사용한다.

## 입력과 공개 계약

`/api/v1/ws`는 정확히 같은 HTTPS Origin과 서비스 쿠키를 검증하고 닉네임이 있는 계정만 입장시킨다. URL/query/subprotocol에 토큰을 넣지 않는다. 매치 생성은 내부 MatchRegistry port만 가능하며 외부 임의 보드/seed/seat 생성 API는 없다. 실제 큐/친구 방·검증된 보드 공급은 #17이 연결한다.

게임 입력은 `{v,match_id,command_id,client_seq,session_epoch,known_revision,action}`이며 action은 기존 PublicAction이다. UUID는 nil 없는 canonical 소문자, sequence/epoch는 양의 안전정수, cell은0..255, unknown/duplicate field·8KiB 초과·target/time/seat는 거절한다. 클라이언트 시각은 받지 않는다. snapshot/ack/error/match_end는 본인의 server_seq와 공개 revision·server_time_ms만 포함한다. 공개 타입은 Rust→생성 TS로 관리한다. 내부 ingress/analysis revision·상대 cells·seed·lie/target/truth는 내보내지 않는다.

모든 입력은 bounded mailbox 입장 시 서버 monotonic clock으로 표시한다. 입장 순서와 시각 부여는 같은 lock에서 직렬화한다. timer도 같은 순서를 사용하며 수신 전에 deadline을 넘긴 입력은 core가 거절한다. duplicate command는 같은 core acknowledgement를 돌려주고 공개 counter가 아닌 내부 ingress 번호로도 클라이언트 승자를 정하지 않는다. 상대에게 보이지 않는 flag/공격만 바뀌면 상대에게 빈 delta를 보내지 않는다.

## 책임과 제한

MatchState는 RuleEngine/참여 seat/공개 projection과 종료 결과를, actor는 mailbox/timer/증명 완료를, registry는 매치/연결 capacity와 ownership을 맡는다. Clock·ResultRepository·SessionReader·SessionInvalidator가 application port다. SQL·WS는 adapter이며 core는 새 인프라를 알지 못한다.

증명 refresh는 global semaphore와 core SolverBudget으로 제한한 spawn_blocking에서 수행한다. 종료/시간 초과/관측 변경의 결과는 commit하지 않는다. 이미 시작한 blocking 작업의 abort가 CPU 실행을 멈춘다고 가정하지 않으며 permit을 작업 종료까지 유지한다. mailbox·송신·결과 대기에도 상한과 deadline을 둔다. 세션당 token bucket 평균20명령/초·burst40, frame8KiB, server 출력에는 별도 상한을 적용한다. 초과/송신 지연은 안정 error/close이며 다른 매치를 막지 않는다.

capacity는 환경 설정으로 주입하며 개발 시험값을 출시 동접 수로 표현하지 않는다. 미니 PC의 실제 admission limit은 #26 실측 뒤 정한다. 종료 actor와 연결은 ownership token을 대조하여 늦은 drop이 새 연결/매치를 해제하지 않게 정리한다. heartbeat15초/45초 무응답, disconnect 이후 core grace는 기존 규칙을 따른다.

인증을 설정한 main은 같은 AuthorityRegistry를 인증 router/Apple maintenance와 WS에 주입한다. PgSessionReader는 서비스 세션 port를 통해 저장소를 조회한다. 개발 기본 제한은 매치16·mailbox64·송신16·proof2·물리 연결32이며 `LIAR_ONLINE_MATCHES/MAILBOX/OUTGOING/PROOF_WORKERS/CONNECTIONS`로 변경한다. 잘못된 설정은 시작을 거절한다. replacement handshake의 최대2초 동안 새 attach를 기다리고, 성공하지 않으면 기존 epoch의 core disconnect를 처리한다. 종료 결과 완료 통지는 입력 mailbox와 독립된 단일 bounded slot으로 보존하여 입력 과부하가 저장 상태를 영구 pending으로 만들지 않게 한다.

## 인증 철회와 원자 결과

인증 snapshot을 읽는 handshake 시작 때 registry generation을 잡고 응답 후 같은 generation에서 연결을 등록한다. 그 사이 logout/삭제/회전이 있으면 거절한다. account별 한 active connection이며 새 연결 epoch가 이전 socket과 queued command를 무효화한다. raw token/hash는 private authority에만 있고 DTO/로그/URL/DB 결과에 넣지 않는다. 세션 만료도 입력과 heartbeat에서 검증한다.

PgAuthStore에 SessionInvalidator를 주입한다. logout·session 회전·account 철회·연결 해제/삭제·유효 Apple 철회는 인증/잠금 확인 후 DB commit 전에 메모리 권위를 먼저 철회한다. 실패/rollback에서도 socket은 보수적으로 닫고 재인증하도록 한다. 이를 actor 적용과 같은 동기 authority lock으로 직렬화하여 이미 철회된 명령이 commit하지 않게 한다. 매 클릭마다 SQL을 수행하지 않는다. DB 계정 삭제와 새 결과 저장은 account FK/동일 transaction 경계로 개인정보를 재생성하지 않는다.

최종 결과는 match UUID unique 부모와 본인 account FK cascade 참여 행을 한 transaction으로 저장한다. 중복 저장은 기존 부모를 바꾸거나 삭제된 참여자를 다시 넣지 않는다. 닉네임은 결과에 복사하지 않는다. seed는 서버 전용 bytea이며 공개 결과 DTO와 분리한다. 저장은 입력 경로 밖에서 제한된 재시도/대기 상태로 처리하고 장애 결과를 일반 승패로 조작하지 않는다. seed/replay와 기록 보존·백업 운영은 #25에 이어 검증한다.

## 검증과 남은 작업

fake clock의 동시 입력/정확한 deadline/숨겨진 상대 flag·중복 결과, 실제 WS Origin/cookie/크기/rate/대체 epoch/느린 송신/권위 철회, 실제 PostgreSQL 원자 결과와 삭제/저장 경합을 실행한다. 성능20ms·실제 miniPC 동접·실제 OAuth 제공자 성공을 단위 시험으로 주장하지 않는다. 매칭17·클라이언트18·공식 기록19·운영25/26은 후속 작업이다.

[axum WS](https://docs.rs/axum/latest/axum/extract/ws/index.html), [frame/message limits](https://docs.rs/axum/latest/axum/extract/struct.WebSocketUpgrade.html), [Tokio blocking/abort 한계](https://docs.rs/tokio/latest/tokio/task/fn.spawn_blocking.html), [bounded mailbox reservation](https://docs.rs/tokio/latest/tokio/sync/mpsc/struct.Sender.html)을 구현 전 확인했다.
