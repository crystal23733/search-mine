# ADR0062 · 공식 데일리 도전·본인 결과·검증 공유

- 상태: 채택 · 승인된 M3/#19의 하위 #132
- 선행: #130/PR131 develop2b0f60f 병합·#130 CLOSED 실제 확인
- 요구: FR01/09/10/11/16, NFR01/02/05/06/07/08, TS17/18/19/20/21/22/26/29/31/35/36

## 결정

기존 무계정 `/daily` 연습과 공개 `/daily/rankings`에서 `/{locale}/daily/official`로 명시 진입한다. 공식 화면은 로그인·nickname·현재 연결이 필요하며 현재 계정/revision을 담은 확인 창에서 시작한다. 로그인 복귀는 기존 허용된 daily 경로를 사용하고 자동 시작하지 않는다. 별도 lazy 화면/controller는 기존 typed DailyApi, AuthPort 전후 proof, DailyCore/LocalController, records/activity/share 포트를 주입받는다. 새 규칙/시간 판정/프로토콜/의존성/DB migration을 만들지 않는다.

start의 날짜/규칙은 서버가 선택한다. captured account/revision과 attempt UUID를 사용하고 응답 accepted metadata/rules로 WASM을 초기화한다. 반환한 전체 metadata와 rules hash 불일치는 board 공개/입력 전에 실패한다. 같은 시작의 응답 소실 재시도는 같은 attempt를 유지한다. 언어와 UTC 자정은 진행 중 run을 교체하지 않으며 다음 명시 시작에서 새 서버 날짜를 받는다. LocalController의 단조 clock/입력 직렬화/상한과 Rust 종료 판정을 재사용한다.

서버가 반환한 heartbeat_interval_ms로 단일 순번/무중첩 heartbeat를 실행한다. 브라우저 timer 범위를 벗어난 interval은 시작 실패이며 수신 ACK를 공식 시간으로 표시하지 않는다. ack:null 또는 현재 요청 실패는 시간 자격을 복구 추정하지 않고 이후 제출 ticket을 null로 한다. 완료 시 timer를 멈추고 이미 진행 중인 heartbeat가 종료된 뒤 제출한다. 서버가 receipt의 verified_elapsed_ms/trust를 결정한다. client가 45초 정책이나 시간 검증을 재구현하지 않는다.

계정/revision/연결 소실·offline·화면 이탈은 진행 요청/timer/core를 중단하고 private 결과를 지운다. 공식 대전의 자동 로컬 권위 전환이나 자동 재시작을 만들지 않는다. 기존 `/daily` 로컬 솔로 연습은 독립적으로 오프라인에서도 가능하다. 이미 완료된 replay는 아래 unverified 후보로 보존한다. 복구/로그인만으로 대기를 전송하지 않고 명시 재시도 또는 설정의 계정 확인을 사용한다. 이탈 dispose는 layout cleanup으로 즉시 실행한다.

clear 완료는 전체 metadata가 일치하는 replay를 읽어 기존 무계정 unverified first record/pending 저장을 먼저 시도한 뒤 시작 때 동의한 현재 owner로 자동 검증 제출한다. 저장 실패는 warning과 탭 메모리 후보를 유지하며 제출을 막지 않는다. 미완료/시간 종료는 제출하지 않는다. 실패/취소는 pending을 유지한다. 명시 재시도는 같은 attempt/replay이고 Practice는 원래 receipt를 표시한다. post-proof 성공 뒤 현재 attempt에 해당하는 pending만 제거하며 개인 기록이나 다른 첫 local attempt를 삭제하지 않는다.

본인 기록은 명시 읽기에서 현재 UTC 날짜의 accepted definition→own을 조회한다. 정상 receipt:null과 실패를 구분한다. 결과에는 서버 receipt의 날짜·실수·완료 신뢰·nullable 서버 시간만 공식 값으로 표시하고 현재 연습 board/time과 분리한다. 원래 첫 receipt가 현재 attempt와 달라도 nickname/로컬 값으로 바꾸지 않는다. 계정 변경·오프라인·늦은 응답은 이 결과를 제거한다. 새 저장소에 account/session/ticket/receipt를 추가하지 않는다.

완료 결과가 확정된 뒤 보드 옆 안내도 receipt의 신뢰를 표시한다. 제출 재시도용 ticket 자격 메모리를 이유로 이미 확정된 결과에 시간 검증 진행 중 안내를 남기지 않는다.

공유는 기존 allowlist DTO를 확장하여 unverified, verified_completion, verified_time을 구분한다. 공식 share는 검증 receipt와 해당 accepted rules에서 날짜/공개 버전·완료/안전 칸 수·실수·시간/신뢰만 선택한다. verified_completion의 replay 시간은 개인 참고이며 공식 시간으로 표시하지 않는다. PNG와 text 모두 같은 신뢰를 표시하고 mosaic는 진행 비율의 장식32칸이다. nickname/account/attempt/ticket/completion ID·seed/hash/정답/숫자/좌표/입력은 포함하지 않는다. 공유 버튼은 네트워크/제출을 호출하지 않는다.

## 검증과 후속

기존 HTTPS 화면에 공식 도전 진입점이 없는 행동 Red를 먼저 확인한다. fake clock/core/API/session으로 accepted binding·start/submit 중복·heartbeat 순번/무중첩/null/실패·late 응답·dispose/계정/연결·미완료·save 경고/첫 후보/실패 유지/같은 attempt retry·원래 Practice receipt·own 없음/실패·공유 allowlist/시간 신뢰를 검사한다. 실제 production HTTPS/PG/WASM의 명시 확인→실제 board 입력→서버 첫 완료/Practice·본인 결과/공유·8언어 PC/mobile·실패 pending/명시 재시도를 검증한다. full·관련95/전체80·문서/Mermaid·코드 리뷰·최신 head CI·squash merge·CLOSED 후 #19 전체 출구를 검토한다.

제공 디자인11/12의 판/결과·공유 구분과 공통 atomic UI/토큰을 따른다. 기존 승인 FR09의 구현이며 개인정보/광고/동의 정책·공정성·서버 시간 규칙을 확대하지 않는다. 실제 OAuth42·운영25/26·사람27·원인 미확정83은 별도다.

브라우저 검증은 서버 response를 실제 전달하기 전에 관측해 같은 response를 fulfill하는 ADR0060 방식을 재사용한다. 가상 clock을 멈춘 검증에서는 공통 Dialog의 paint 후 effect와 실제 WASM worker의 비동기 advance를 실행·관측하며 시계를 진행한다. 단순 시간 경과로 worker 완료를 가정하지 않고 공개 셀의 기절 해제를 관측한다. 두 실제 판을 입력하는 시나리오의 전체 상한은 120초이며 기존 공통 timeout/retry/제품 timer는 바꾸지 않는다.
