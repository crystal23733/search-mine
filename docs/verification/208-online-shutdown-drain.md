# #208 정상 종료의 매칭·판·저장 drain

[ADR0096](../adr/0096-online-shutdown-drain.md) · FR03/06/07/14/16·NFR01/02/05/07/08·TS15/16/20/21/28/31/35/36.

선행206/PR207 종료·최신develop0edda8b clean/원격동기화를 확인했다. 제품 코드 전에 종료 순서·상한·실패 정책·시나리오를 기록했다. 실제 Red/Green·검증·리뷰·PR/병합은 실행 후 기록한다. 실제 Docker/Tunnel/호스트·부하·백업/복구·#83은 별도 미완료다.


## 실제 Red/Green

- 실제 owned main/PG/HTTP/WS Red: 진행 판을 준비하고 SIGTERM을 보낸 뒤 ready 조회가 IncompleteMessage로 실패했다. 기존 구현이 먼저 HTTP/프로세스를 종료했다. `.tmp/drain208-red.log`와 개별 main 진단을 보존했다.
- Green 실제 child: 준비된 온라인 판/기존 WS를 유지하며 ready503·live200·새 queue503·동일 본인 재접속 snapshot을 확인했다. active journal row를 실제 transaction으로 잠가 결과 Pending을 관측한 동안 프로세스 생존/결과404를 확인하고, lock 해제 뒤 정상exit·원래 cancelled 결과 저장·active row0을 확인했다(4.30초). 240초 실제 벽시계 대전을 이 테스트에서 기다리지는 않았다.
- 실제 actor/Rust core는 제어 clock으로 정상 입력 ACK·재접속 cursor·240초 종료를 진행했다. account 점유 해제 뒤에도 저장 port 대기 동안 drained=false, Saved 관측 후에만true·Failed 관측은 오류임을 확인했다. 결과 전달 TTL30초가 남아도 저장 완료를 분리했다. 네트워크의 최종 메시지 수신 ACK를 보장한다고 주장하지 않는다.
- 실제 CPU prep을 gate로 막은 취소·queue 해제·permit 유지/반환, 시작된 register 대기·ACK 뒤 discard/permit 반환을 검증했다. 새 registry create도 drain 이후 Unavailable이다. paused Tokio로300초 drain/5초 HTTP 상한·실패 전파와 drain 전 HTTP 종료 금지를 확인했다. 실제 정상 runtime CPU cleanup 상한과 기존 fatal cleanup도 검증한다.
- 추가 Red: 명시 fail_closed 직후 worker abort가 아직 스케줄되지 않았을 때 available=true가 남는 경합이 실패했다(`.tmp/drain208-owner-red.log`). 동기 sticky reject·완료 직전 재검증으로 Green을 확인했다. server future가 마지막 poll에서 reject하는 경합도 오류로 보존한다.
- 실제 PG 전체187 PASS(17.16초): 새 main 정상 drain 및 drain 중 owner backend 종료→오류exit/원래 journal 보존, 기존 HTTP/WS·권한·계정/동의/이벤트·보존·재시작 복구를 포함했다. 최종 source 전체Rust/clippy·문서/Mermaid·CI 결과는 실행 뒤 PR/로컬review에 기록한다.

## 실제 코드 리뷰와 한계

로비 lock→registry lock 순서로 gate를 닫으며 기존 authority lock 안의 status 조회·철회 재검증을 유지한다. 준비 receipt/CPU permit 수명은 기존 정리 경로를 재사용하고 drain 중 tick/dispatch로 새 job을 만들지 않는다. unresolved ID는 create와 같은 lock 안에서 등록하고 저장 결과를 actor가 관측·발행한 뒤에만 제거한다. 실패는 sticky이며 actor 비정상 종료/저장 오류를 정상 완료로 바꾸지 않는다. 300초 초과는 취소/오류 종료 후 기존 journal 복구에 맡긴다. owner guard는 signal drain 전체와 마지막 결과를 감싼다.

공개 admin/포트·protocol/규칙·게임시간·8언어·광고·동의·의존성/lockfile·browser timeout/retry/flags를 변경하지 않았다. 실제 Docker/Tunnel/호스트와 실장비307초 정리·실Windows/네 제공자 계정/부하·RPO/RTO는 미실행이다. native 신호·paused 상한/port 성공을 실제 운영 검수로 바꾸지 않는다. #24/#25/#26/#42/#83 및 외부출구는 남는다.

최종 로컬 검증: workspace Rust476 PASS, clippy workspace/all-targets -D warnings·fmt/diff PASS. 실제 main 관련7개를 최종 status/authority 경계까지 다시 실행해 PASS(4.39초)했다. 문서254·제품21·설계18·시나리오36/Mermaid34 PASS다. 로컬 LLVM/전체브라우저·Windows/Docker 운영검수는 미실행이며 원격 필수6CI를 별도 확인한다.
