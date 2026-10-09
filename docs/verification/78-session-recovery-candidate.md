# #78 연결 장애의 비권위 세션 복구 후보

FR01/14/NFR01/02/05 · TS15/16/20/31/35/36 · [ADR0037](../adr/0037-session-recovery-candidate.md). #76/PR77/developc25181e3 병합·종료 뒤 시작했다.

## 구현과 실제 실행

SessionRecovery는 원래 account/session/revision을 메모리의 단일 비권위 후보로 최대30초 보존한다. suspend는 공개 계정/identities/CSRF를 지우고 진행 중 요청을 abort한다. 후보에는 권한이 없고 connected false/execute·쓰기 불가다. resume는 전/후 bootstrap과 identities에서 같은 원래 session을 확인해야 복원하며 accepted nickname은 마지막 응답을 따른다. 개별10초/동시1·전체 단조30초·역순/abort 무시·refresh/invalidate/쓰기/dispose·guest/미완료 nickname 경계를 검증한다. HTTP fetch/읽기 거절·timeout 분류는 실제 네트워크 원인의 확정이 아니며 잘못된 수신 DTO/JSON/401과 구분한다.

새 API 부재7failed(1.76s)와 기존 browser offline의 revision3≠2인1failed(1.74s)를 Red로 실행했다. 초기 관련 인증41Green(2.57s), 확장51Green(2.53s)·typecheck/lint였다. correctness 리뷰에서 복구 후 같은 revision에 도착한 옛 데일리 ACK가 대기를 지우는 충돌을 확인했다. ADR을 먼저 보완한 뒤 actual auth와 pending port로1failed/16passed Red(1.84s)를 재현하고, SubmissionSession 구독으로 중단을 기억하고 signal을 abort·late reply를 버리도록 보정했다. 관련 인증/제출56Green(2.61s)이며 이전 execute의 같은 revision 복구 후 late response도 취소 상태를 유지한다.

실제 HTTPS 첫 시험1failed/1미실행은 테스트 로그아웃 POST의 필수 빈 JSON 누락(400)이었다. 요청을 제품 계약과 맞춘 PC/mobile2Green(7.1s)을 확인했다. 이후 거절 오류 코드/새 proof 수신 검증을 강화하면서 이전 build를 사용한1failed/1미실행은 성공으로 세지 않았다. 최종 fresh build의 실제 HTTPS PC/mobile은 각각970/942ms로 통과했다. 키 없는 fixture는 실제 Rust router/PostgreSQL17.4/HTTPS HttpOnly cookie를 사용하고 외부 provider proof만 시험 port다.

최종 scripts/check.ps1은 locked fmt/all-target Clippy/native/WASM·생성 타입/fixture/tokens·웹 format/lint/typecheck/unit/build·실제 HTTPS 포함81browser(2.2m)·문서131/Mermaid34오류0을 통과했다. 웹 line coverage2060/2353=87.54%다. 기본 native의 ignored DB를 실제 DB 성공으로 계산하지 않는다. CI PostgreSQL18·최신 head/병합은 PR에 기록한다.

## 리뷰와 남은 출구

correctness scope78/basec25181e3로 후보와 권위·revision과 요청 generation·실제 cookie session 교체·accepted nickname·시간 예산·구독하는 소비자들을 순차 검토했다. 마지막 서버 nickname이 제안/첫 proof와 다른 경우와, 지연 proof를 살아 둔 채10초 뒤 다른 시도를 먼저 완료한 경우를 강제로 분리해 확인했다. 데일리 제출의 중단/복원 충돌은 실제 Red로 재현하고 수정했다. 추가로 뒷받침되는 결함은 없었으며 별도 성능·보안 감사를 했다고 표현하지 않는다. WS backoff/미확인 command UUID 재전송·복구 중 보드 유지·영속 journal/재시작 abort/결과 조회는 후속 #18이며 이 후보의30초가 Rust 서버 grace나 승패는 아니다. 실제 OAuth 키42/하드웨어26/사람27은 별도다.
