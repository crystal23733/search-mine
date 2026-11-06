# #206 · 정상 종료 신호

[ADR0095](../adr/0095-normal-shutdown-signals.md) · FR16·NFR01/02/05/07/08·TS20/21/28/31/35/36.

선행204CLOSED/PR205MERGED·최신develop3d8a475 clean/origin0:0을 확인했다. 상위24의 실제 호스트·전체 drain과 독립적인 SIGTERM 기반을 분리하고 제품 코드 전 설계/시나리오를 기록했다. Red/Green·실제 검증/리뷰·CI/병합은 실행 후 기록한다. 실제도메인/계정/호스트·Docker stop/drain·83은 별도 미완료다.


## 실제 Red/Green·회귀

- 실제 main child를 loopback liveness로 관측한 뒤 owned PID에만 신호를 보냈다. 기존 SIGINT는 통과했고 SIGTERM은 ExitStatus(unix_wait_status(15))로 정상 exit 기대가 실패했다. raw `.tmp/shutdown206-red.log`에 보존했다.
- main의 private factory가 Unix SIGINT/SIGTERM stream을 HTTP bind 전에 설치하고 같은 axum graceful shutdown future에 연결한다. 설치 실패는 안정 오류로 기존 fatal 경로에 반환한다. nonUnix Ctrl+C 경로는 그대로다.
- Green: 두 신호와 기존 standalone runtime 5개 PASS(1.80초), clippy workspace/all-targets -D warnings 및 workspace Rust 전체 통과. 실제 PostgreSQL ignored185 PASS(18.76초): 기존 main owner-loss/HTTP/WS·중복 소유·계정/동의/보존 회귀를 포함했다. 로컬 LLVM/실 Windows/Docker·활성 판 drain은 실행하지 않았다.
- 문서252/제품21/설계18/시나리오36·Mermaid34·fmt/diff 검사를 통과했다. Cargo 의존성/lockfile·프로토콜·게임/인증·기존4초 및 요청기한·CI/browser flags/retry를 바꾸지 않았다.

## 코드 리뷰

signal factory는 실제 main의 composition root에서 한 번 호출한다. signal 등록/실패는 listener bind 전에 끝나고 stream은 shutdown future가 소유한다. owner-loss watcher와 ADR0051 fatal runtime 경로를 그대로 유지했으며 per-request handler·새 endpoint/포트·사용자값 로그를 추가하지 않았다. 테스트는 기존 env_clear/child guard를 재사용하고 양수 child PID에만 /bin/kill을 보낸다. 실패에도 own child kill/wait를 실행하며 LLVM_PROFILE_FILE만 기존 PG helper처럼 추가 전달해 계측 프로파일을 보존한다.

추가 수정 필요 사항은 발견하지 않았다. 4초 정상종료 증거는 무작업 standalone 프로세스의 검사이며 실제 활성 게임/DB drain·전체 종료 상한/운영호스트 성공은 상위24에서 확인한다. 필수6CI/원격coverage·정확head리뷰·병합은 PR/로컬review에 기록한다. 실제Windows실행·Dockerstop·83을 완료 판정하지 않는다.
