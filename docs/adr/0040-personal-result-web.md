# 0040. 메모리 배정 판의 본인 저장 결과 확인

상태: 채택 · M3 #18/#89. 선행 #82/PR88 develop cdd700e 병합·종료 확인. FR11/14/16, NFR01/02/05/06/08 · TS15/16/20/21/26/31/35/36.

## 명시 조회와 수명

현재 화면의 메모리 `matched.match_id`와 연결된 현재 인증 소유자만 조회한다. 자동 재접속이 실패한 error 상태 또는 종료 결과의 저장 알림이 saved가 아닌 상태에 “저장 결과 확인”을 제공한다. 게임 진행/재접속 중에는 조회하지 않으며 후보만 있는 offline 상태도 요청 권한이 없다. 조회 동시1, 자동 poll/retry 없음. 서버 not_found는 미존재/다른 계정/삭제/만료를 구분하지 않는 안내이며 pending/승패/완료를 추정하지 않는다. 그 외 실패는 확인 불가와 수동 재시도를 제공한다.

`OnlinePort.result`와 DI HTTP adapter는 기존 AuthenticatedRequests의 fresh bootstrap 전후 proof·CSRF·same-origin cookie·no-store·redirect:error·전체10초/취소를 사용한다. 최소 결과 body는 최대4096byte로 읽으며 strict 생성 타입 decoder가 버전/UUID/저장 hash/정수 범위/정확한 키·요청 match 상관관계를 검증한다. 서버가 준 reason/outcome/completed를 사용하며 웹에서 완료나 승패를 계산하지 않는다. 현재 bundled rules를 붙이지 않는다.

조회는 세션 revision·controller generation·개별 AbortController에 묶인다. 다음 판/start, 화면 이탈, 권한 교체/취소, WS 종료 결과가 먼저 도착하면 진행 조회를 취소하고 늦은 응답을 폐기한다. HTTP 검증 성공을 커밋하면 기존 WS/재접속/명령을 종료하고 완전한 보드 없이 최소 저장 결과를 표시한다. 그 전에 수신한 WS 종료 결과가 우선한다. 이미 표시한 결과도 offline 전환/권한 상실 시 제거한다. 계정/match/결과를 일반 storage에 쓰지 않는다.

## 화면과 검증

정상 완료 actor의 30초 정리는 계정 세션 철회가 아니다. 기존 lease release와 session/account invalidation이 같은 watch 신호로 닫혀 transport가1008로 처리하던 경계를 구분한다. registry lock 안에서 현재 lease의 정상 release 사실만 표시하고, WS 종료는 그 경우1000을 사용한다. 교체/만료/logout/삭제는1008과 기존 권한 차단을 유지한다. 이 표시는 권한을 부여하지 않으며 모든 결과 요청은 독립된 fresh proof/서버 session 검증을 다시 거친다. release와 철회가 경합하면 먼저 registry에서 제거한 원인을 따르고, 정상 release 뒤 철회도 이후 HTTP 인증이 차단한다.

design_layout/10 결과.png의 제목·reason·카드·통계·주요 다시하기/홈과 공통 Atomic 토큰을 따른다. 최소 결과는 본인 opened_safe·mistakes·correct/attempts만 표시한다. 상대 값·보드·전체 안전칸 분모·남은 시간을 만들지 않는다. 8언어, 키보드, PC/mobile와 긴 문구를 검사한다. 저장 결과 확인은 새 게임 완료가 아니므로 광고/이벤트/완료 횟수를 추가하지 않는다.

decoder/인증 HTTP·controller의 역순/취소/WS 우선·offline/교체를 Red→Green으로 확인한다. loopback fixture도 production PgResultReader/result_router와 세 번째 철회 registry를 조립하고 실제 PostgreSQL/HTTPS에서 최종 알림 유실·30초 actor 정리 후 조회와 uniform404·권한 실패·최소 DTO/storage0/8언어를 검증한다. fixture clock 제어는 운영 경로에 추가하지 않는다. 새로고침 후 잃어버린 match 발견·영속 active journal·재시작 abort는 후속 #18이다. #83 원인과 외부 OAuth/장비/사람 출구는 미해결로 유지한다.
