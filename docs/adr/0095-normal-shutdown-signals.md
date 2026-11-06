# ADR0095 · 정상 종료 신호의 composition root 연결

- 상태: 채택 · #206(상위24) · FR16·NFR01/02/05/07/08 · TS20/21/28/31/35/36
- 승인: 초기 운영 설계/FR16·2026-10-07 전체 순차 위임/외부 입력 이연·현재 진행 지시. 선행204/PR205 MERGED/CLOSED, develop3d8a475.

## 관측·결정

실제 main은 Ctrl+C만 graceful_shutdown에 연결한다. [Docker stop](https://docs.docker.com/reference/cli/docker/container/stop/)의 기본 SIGTERM과 기존 SIGINT를 Unix에서 같은 정상 경로에 전달한다. main의 private signal factory가 HTTP listener bind 전에 두 Tokio Unix stream을 설치하고 future를 반환한다. 설치 실패는 안정된 오류로 기존 fatal 경로에 반환한다. nonUnix는 기존 Ctrl+C 처리를 유지한다. process-wide handler는 per-request로 만들지 않는다. [Tokio Signal 계약](https://docs.rs/tokio/latest/tokio/signal/unix/struct.Signal.html)을 따른다.

계정/DB/게임 DTO·기본bind·인증/Origin·owner-loss/fatal runtime(ADR0051)과 실제입력/WS복구는 변경하지 않는다. endpoint·포트·사용자값/secret 로그·의존성을 추가하지 않는다. 신호는 axum의 기존 graceful_shutdown을 시작하며 실제 진행 판 drain/새 매칭 중지·운영 종료 상한/실제 Docker stop 검수는 상위24의 별도 출구다. 무작업 standalone의 정상 종료 검증을 운영 활성 게임 drain 성공으로 부르지 않는다.

## 실제 검증 계획

기존 lobby_runtime의 env_clear/Server child cleanup/loopback health/live·4초 준비 패턴을 사용해 실제 main child만 띄운다. child 준비를 관측한 후 그 양수PID에만 /bin/kill SIGTERM/SIGINT를 보낸다. 4초 안의 정상 exit status를 검사하고 실패에도 own child를 kill/wait한다. 부모 테스트 프로세스에 신호를 보내지 않는다. LLVM_PROFILE_FILE은 기존 PG helper처럼 명시 전달해 계측 child 결과를 보존한다.

먼저 기존 SIGTERM의 OS 신호 종료 Red와 기존 SIGINT 행동을 확인하고 최소 구현 뒤 Green으로 확인한다. fmt/clippy/workspace·기존 fatal runtime·실PG owner-loss/HTTP/WS·전체6CI/coverage를 검증하고 실제 코드리뷰/정확head 병합한다. 실제 Windows/Docker/호스트·RPO/RTO/drain과83을 이 native 증거로 완료 처리하지 않는다.
