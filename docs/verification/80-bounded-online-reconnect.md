# #80 제한된 자동 재연결·미확인 명령 복구

FR14/NFR01/02/05/06 · TS15/16/20/21/26/31/36 · [ADR0038](../adr/0038-bounded-online-reconnect.md). #78/PR79/developa223bd4f 병합·종료 뒤 시작했다.

## 구현과 실행 근거

BoundedReconnect는 최초 단조 시각부터 전체30초·개별10초·0.5/1/2/4/8초 cap+25% jitter·동시 시도1개를 소유한다. 현재 인증 권한 또는 같은 원래 세션의 비권위 후보에만 복구를 시도하며 실제 연결에는 fresh proof를 요구한다. 후보 동안 OnlineEntry의 identity key와 공개 보드를 유지하되 입력은 차단한다. 연결 lease를 교체 전에 폐기하고 늦게 반환된 연결은 자기 연결만 닫는다. 성공한 연결은 재시도 timer 정리로 abort되지 않는다.

최대16개의 원본 명령은 메모리에만 보존한다. snapshot의 같은 매치/rules·감소하지 않은 revision·새 epoch를 확인하고 서버 소비 cursor와 이미 예약한 client_seq 중 큰 값을 사용한다. UUID/seq/action/known_revision은 유지하고 epoch만 교체해 재전송한다. ACK의 원래 revision이 현재 view보다 낮아도 소비하며 보드 효과를 낙관 적용하지 않는다. 재전송 뒤 ACK10초 상한은 무한 복구를 막는다. terminal result·권한 변경·탐색/dispose·예산 소진은 명령을 폐기한다.

loop 최초2failed/1passed(1.69s)에서 브라우저 deadline timer가 늦을 때 기한 후 성공을 받는 결함과 delay 기대 횟수 오류를 구분했다. 단조 기한 검사를 보완했고 계획 delay의 실제6회(29.375초 포함)로 oracle을 보정했다. 컨트롤러/loop22Green(2.57s) 뒤 OnlineEntry 오프라인 보드 유지1failed Red(9.09s, gridcell 부재)를 확인해 UI를 연결했다. 관련23Green(9.30s)·snapshot 오염/정수 상한/종료 확장28Green(10.07s)을 확인했다.

실제 PostgreSQL17.4·Rust router/WS·HTTPS cookie 환경에서 PC1440/mobile390의4개 시험이48.3초에 통과했다. Playwright route는 실제 서버와 통신하면서 적용된 ACK 하나만 버리고 연결을 끊었다. 복귀 snapshot의 새 epoch/소비 cursor/flag1회·동일 원본 명령 cached ACK/원래 revision·seq+1 새 명령·시간 진행·SQL Saved를 확인했다. 실제 navigator offline은 보드를 읽기 전용으로 유지하고 동일 세션의 fresh proof 뒤 복귀했다. 두 번째 offline 동안 다른 context가 실제 DB logout한 세션은 복귀하지 못했고 화면/입력을 지웠다. 일반 storage에 매치/명령 ID·온라인 secret을 기록하지 않았다.

8언어 output/progress·입력 정지·모바일/PC overflow0과 보드 앞 안내 순서를 검사했다. [원본 오류 상태](../../design_layout/15%20%EC%98%A4%EB%A5%98%20%EC%83%81%ED%83%9C.png)와 실제 `.tmp/online-layout/80-recovery-{390,1440}.png`를 직접 시각 비교했다. 로컬 시도 제한은 서버 grace/승패 보장이 아니라는 문구를 사용했다.

## 코드 리뷰·남은 출구

pm-skills correctness, scope80/basea223bd4f로 auth 후보→fresh proof→WS 수명→snapshot→명령 cache→결과 소비자를 순차 검토했다. 서버가 cursor9를 승인한 경우 다음 입력10, view8에서 원래 ACK7, 정수 상한의 cached ACK 복구를 강제로 확인했다. 30초에 실패한 connect를 살아 둔 채 새 연결을 먼저 성공시킨 뒤 늦은 원래 연결이 새 연결을 닫지 못하는 실행도 확인했다.

리뷰에서 복구 연결만 saved/failed 뒤 close를 새 disconnected 오류로 바꾸는 불일치를 발견했다. ADR을 먼저 보완하고2failed/24passed Red(5.19s)로 재현해 최초 연결과 같은 완료 처리로 수정했다. 관련30Green(10.23s)·typecheck/lint를 확인했다. 전체 gate/CI·최신 head·PR 병합은 완료 후 기록한다. 별도 성능·보안 감사를 했다고 표현하지 않는다.

서버 결과만 신뢰하며 로컬30초로 forfeit/abandon/Saved를 만들지 않는다. reload 후 미확인 UUID 복원은 지원하지 않는다. 영속 active journal·서버 재시작 abort·인증 결과 조회와 상위 #18 통합 출구는 후속 작업이다. 실제 OAuth42·장비26·사람27은 별도다.

첫 로컬 전체 게이트는 Rust fmt/Clippy/native/WASM·생성 타입/fixture/tokens·웹 format/lint/typecheck까지 통과한 뒤 전체 unit176개 중5개 실패(171pass,36.11s)로 종료했다. 친구/큐는 동적 페이지 로딩 중 기본 findBy 제한, 기존 로컬2개는5000ms 전체 제한에서 실패했다. 같은 기본 명령 재실행도5fail(34.78s), worker수만4로 제한한 비교는2fail/174pass(59.08s)이었다. `.tmp/online80-unit-repeat.log`와 `.tmp/online80-unit-four.log`를 보존했다. 부하 원인을 확정하지 않고 제품/테스트 timeout·retry·worker 설정을 바꾸지 않았다. 로컬 전체 게이트를 성공으로 계산하지 않으며 독립 CI의 전체 unit·DB18·browser와 최신 head 확인 후에만 병합한다.
