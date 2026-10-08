# #16 · 권위 매치 actor·WebSocket 검증

FR02~05/15 · TS14/20/21/22 · [ADR0023](../adr/0023-authoritative-match-actor.md) · 선행 #46 PR50/develop684673e5 병합·종료. #16 PR51에서 Rust RuleEngine을 bounded actor로 감싸고 서비스 세션으로 인증한 `/api/v1/ws`, 비동기 원자 결과 저장을 추가한다.

## 실제 실패와 수정

| 행동 | 관측한 Red | 수정과 재검증 |
|---|---|---|
| logout/삭제 시 기존 WS 권위 철회 | f1640ff의 PostgreSQL18에서 기존16 passed·observer2 failed | PgAuthStore commit 전 Invalidator barrier, handshake generation; 08b12c0의 실DB18 passed/ignored0 |
| 결과 중복·삭제 후 retry·두번째 참여자 SQL 실패 | 4f239a9/run37805127905에서 실DB18 passed·결과3 failed(Unavailable) | ab61f7a/run37806244813의 database job113411159088:21 passed/ignored0, unique 부모+두 참여자 한 transaction |
| 실제 runtime의 서비스 세션 조회 | 2e9e884/run37806601839의 job113413149440에서21 passed·1 failed(Stored service session must be available) | PgSessionReader와 main의 공유 Invalidator 구성; d727d68/run37807538756:실DB23 passed/ignored0 |
| 중복 ack의 공개 revision | 후속 flag 뒤 retry가 원본2 대신3 반환 | core가 저장한 명령의 공개 revision만 bounded cache, 같은 ack 반환 |
| 저장 완료 통지 | mailbox admission에 의존하여 Pending 유지 | actor가 단일 oneshot slot을 소유해 완료를 보존, Tick에서 Saved/Failed로 전환 |
| 대전 시간 | 서버 uptime9000ms가 저장 기간에 더해짐(252000 != 243000) | 명시적 created_at, 종료 elapsed만 저장 |
| WS 공통 오류 envelope | malformed 입력의 match_id가 Null | 오류도 actor의 OnlineEvent/server_seq를 사용; 실제 socket 검사 |
| frame 초과 close |8KiB 초과 입력 뒤 명시적 close code 누락 |1002 protocol close, malformed/rate/철회1008 policy close |
| 독립 계정의 동시 handshake | 두번째 정상 등록이 Unauthorized | generation은 철회/만료에서만 변경하고 정상 등록은 ownership으로 직렬화, 권위4개 통과 |

## 계약과 실행 범위

`MatchState`가 비공개 engine/seed/참여자를 소유한다. WS는 Rust public DTO만 전송하며 입력에 seat/clock/seed/target을 허용하지 않는다. 본인의 숫자/변경 이력과 상대 진행·공개 기절만 전달한다. 상대 flag·숨은 공격에 대해 빈 delta/공개 revision 증가를 만들지 않는다. 생성 TypeScript는 Rust 선언에서 나온다.

입장은 server monotonic 시각과 mailbox 순서를 같은 lock에서 결정한다. 명령 처리와 권위 철회도 같은 동기 lock을 사용한다. 세션 snapshot 읽기와 등록 사이 logout이 끝나도 generation이 바뀌어 handshake를 거절한다. 새 연결은 이전 socket/queued 입력을 무효화하고 늦은 이전 close는 새 ownership을 해제하지 않는다.

실제 loopback TCP WS9개: Origin/cookie/nickname·공개 snapshot/ack, 철회 close, 공통 error sequence,8KiB frame,40/41 burst, 물리 capacity, 새 epoch/늦은 close, 읽기 중 logout 경합을 검사한다. actor4개는 단일 종료/저장, 중복 계정·매치 capacity, 실패3회 재시도와 종료30초 정리, 느린 수신자와 다른 seat 진행을 검사한다. pure state11개는 idempotency/원본 ack·deadline·private revision·stun·grace·proof freshness·입력/참여자 경계를 검사한다.

PostgreSQL18의23개는 기존 인증·권리와 추가7개를 실제 격리 schema에서 실행한다. unique concurrent retry, 계정 삭제 뒤 retry/새 결과, 두번째 INSERT의 SQL constraint 실패 시 부모/첫 참여자 rollback, account lock에서 대기하던 save와 삭제 경합을 포함한다. 닉네임·identity·session/token/비밀번호 컬럼을 결과에 복사하지 않는다. 저장에 실패하면 completed 승패는 유지하고 recording Failed를 표시한다.

d727d68의 CI 측정: 인증 도메인369/371(99.46%), 매치 도메인209/209(100%), entry point를 포함한 전체 서버3971/4390(90.46%). 이 결과는 이후 최종 head 검사를 대체하지 않는다. 도메인95%·전체80% gate는 그대로 적용하고 MatchState도 별도95%로 검사한다.

## 실행과 한계

README의 실제 DB/migration 명령과 `scripts/check.ps1`, GitHub Product checks를 사용한다. DB 없는 기본 cargo test의 ignored PostgreSQL 검사는 통과 개수에 포함하지 않는다. 테스트 provider/계정·작은 비공개 보드는 외부 등록 검수42나 실제 매칭 보드17을 대신하지 않는다.

인증을 설정한 main은 WS와 같은 AuthorityRegistry를 HTTP/Apple maintenance에 주입한다. 개발 기본값은 매치16/mailbox64/outgoing16/proof2/연결32이고 `LIAR_ONLINE_MATCHES/MAILBOX/OUTGOING/PROOF_WORKERS/CONNECTIONS`로 설정하며 잘못된 값은 시작 실패다. 매치 공급은 #17에서 연결하므로 현재 임의 공개 보드 생성 API가 없고 매치 없는 계정의 WS는409 not_matched다. 인증 미설정 실행은 무계정 연습을 유지한다.

실제 HTTPS/Tunnel WS, 미니 PC 성능·출시 capacity/20ms, 사람의 이해·재미, 실제 OAuth 성공을 이 검사로 주장하지 않는다. #17 매칭/친구방/봇, #18 온라인 화면/재접속, #19 공식 기록, #25 seed7일/결과90일 보존·복구 운영, #26 부하 측정을 순차 진행한다.
