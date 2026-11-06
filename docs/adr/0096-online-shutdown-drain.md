# ADR0096 · 정상 종료의 매칭 차단과 판·저장 drain

- 상태: 채택 · 승인된 FR03/06/07/14/16·NFR01/02/05/07/08 운영 drain 구체화
- 선행: #206 CLOSED/PR207 MERGED · 상위 #24 실제 배포 출구 유지
- 추적: TS15/16/20/21/28/31/35/36 · #208

## 결정

첫 정상 종료 신호 뒤 로비 변경/advance와 같은 lock 안에서 drain을 시작한다. registry create도 자체 lock의 닫힌 입장 상태를 검사해 직접/경합 생성이 이후 성공하지 않는다. queue/room/preparing 상태와 jobs를 취소하고, admission permit이 모두 반환될 때까지 기다린다. 시작된 register는 기존 receipt의 관측·discard/fail-closed를 유지한다. 이후 신규 로비 변경은 기존 Unavailable로 거절하고 status는 기존 매치 발견/결과 조회를 유지한다. ready는 즉시503/no-store, live는200이며 기존 HTTP/WS 입력·재접속·인증 철회를 유지한다. 공개 관리 endpoint/새 프로토콜/광고·개인정보 변경은 없다.

단순 account 매핑 제거/매치 결과 Pending을 완료로 간주하지 않는다. registry는 create 시 unresolved ID를 등록하고 actor가 저장 결과를 관측·공개한 뒤 해제한다. 최종 저장 실패/예상치 못한 actor 종료는 정상 drain 성공을 막는다. 결과 보관용 actor30초 TTL은 기다릴 필요가 없다. 기존 규칙·240초·재접속 유예/승패·저장3회/2초·fatal owner 우선 처리는 유지한다.

조립 root는 신호를 관측한 뒤 최대300초 동안 기존 판과 admission 정리를 기다린다. bundled240초+countdown/유예·결과 재시도 정리를 위한 운영 상한이며 측정 성능이 아니다. 성공 후에만 axum graceful shutdown을 시작하고 HTTP 마무리 최대5초를 기다린다. drain/HTTP 상한 초과·저장 실패/owner-loss는 오류 종료로 기존 fatal runtime 경로를 따른다. 성공 시 남은 bounded CPU 작업의 runtime 대기는 최대2초로 제한한다. 실제 Docker stop_grace_period는 전체307초보다 여유 있게 설정·별도 검증해야 하며 기본10초 stop으로 완료를 주장하지 않는다. 두 번째 신호를 강제 성공으로 바꾸지 않는다.

[axum graceful shutdown](https://docs.rs/axum/latest/axum/serve/struct.WithGracefulShutdown.html)만으로 앱 actor/저장 완료를 추정하지 않는다. [Tokio runtime](https://docs.rs/tokio/latest/tokio/runtime/struct.Runtime.html)의 blocking task 종료 대기 특성을 고려한다. 외부 계정·장비·Compose/Tunnel·실제 Docker stop·부하/전체 RPO/RTO는 #24~26에 남긴다.

## 검증

기존 port를 제어해 queue/preparing 취소와 register ACK→discard·permit 반환, 생성 경합 차단, 기존 실제 actor 입력/재접속과 저장 Pending→Saved/Failed·예상치 못한 종료를 검증한다. paused Tokio 시간으로300초/5초 상한을 실행한다. 실제 PostgreSQL/main child·HTTP/WS에서 SIGTERM 뒤 ready503/live200·새 매칭 거절·기존 연결·저장 완료 후 정상exit를 확인하고 기존 owner-loss/fatal 회귀를 실행한다. Red→Green 원문과 미실행 환경을 기록한다. #83 flaky는 별도다.

## 실패 요청과 종료 완료의 경합

review 중 receipt cleanup 실패의 fail_closed가 worker abort 예약만 하고 health watch 반영을 기다리는 구간을 확인했다. 명시 fail_closed 및 미관측 started-register 취소는 공유 health에 동기적인 sticky reject를 먼저 기록한다. worker 종료/drop 감지 watch는 계속 유지하고 guard는 server 완료 직전에도 health를 재확인한다. abort 요청과 worker 스케줄링 사이에 정상 drain이 성공으로 빠지는 것을 막는다. 이 경합은 별도 제어된 Red→Green으로 검증하며 자연 CI 실패의 유일 원인으로 단정하지 않는다.
