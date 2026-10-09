# ADR0038: 제한된 자동 WS 복구와 메모리 명령 재전송

2026-10-09 · 승인된 FR14의 구현 구체화 · #80 · #78/PR79/developa223bd4f 병합·종료 후 구현한다.

## 권한·화면·수명

OnlineEntry는 현재 정상 owner 또는 #78의 비권위 recoveryOwner가 같은 경우에만 기존 OnlineSession/온라인 공개 보드/controller를 유지한다. key는 accountId/revision으로 같지만 후보 동안 입력·매칭·공식 로컬 전환 권한은 없다. 공개 account/CSRF는 #78대로 지우고 nickname은 권한으로 쓰지 않는다. 복구 중 UI는 읽기 전용 보드와8언어 연결 안내/로컬 시도 잔여를 표시한다. 후보의 account/session 불일치·명시 revocation/1008·계정 쓰기/refresh·탐색/dispose는 모든 복구와 명령을 폐기하고 권한 변경에서는 보드도 지운다.

controller의 작업 generation과 실제 WS connection lease를 별도로 둔다. 교체 전에 lease를 증가시키고 기존 connection을 닫는다. 늦은 성공/메시지/close는 자기 lease가 현재가 아니면 버리고 반환받은 자기 연결만 닫는다. 같은 계정 후보는 owner identity 수명일 뿐 연결 권한이 아니며 실제 connect/send는 auth.connected와 owner account/revision을 다시 요구한다. 후보가 있으면 resume(expected)의 전/후 fresh proof 성공 뒤 현재 권한을 다시 확인한다. browser resume와 겹치면 단일 후보 시도 완료/다음 backoff를 기다리며 같은 계정을 새 세션으로 바꾸지 않는다.

## 제한된 복구

진행 중 network disconnect/send 실패/첫 ACK timeout/stream gap에서 같은 매치 복구를 시작한다. malformed/허용되지 않은 DTO·명시 권한 철회는 자동 retry하지 않는다. 이미 종료된 판의 결과를 바꾸거나 saved를 추정하지 않는다. match_end pending 뒤 저장 확인 전에 연결이 끊기면 결과/불확실 저장 상태를 보존하며 이후 결과 조회 작업으로 넘긴다.

최초 복구 시작의 단조 시각부터 전체30초, delay0.5/1/2/4/8초 이후8초 cap+최대25% 지터다. 한 번에 시도1개/재시도 timer1개/전체 deadline timer1개다. 각 connect는 기존 인증10초 상한을 가지며 전체 기한이 먼저 오면 abort한다. offline 중에는 connect하지 않고 지연과 예산만 진행한다. 실제 native grace/종료 시각은 서버가 결정하고 로컬 예산으로 forfeit/abandon을 만들지 않는다. 시도 성공은 권위 snapshot을 실제 수신·검증한 경우다. 성공한 연결의 AbortSignal 수명은 controller에 남기고 retry 타이머 정리로 새 연결을 abort하지 않는다. abort 무시 late connect는 현재 lease와 비교해 자기 연결만 닫는다.

최초 연결과 복구 연결 모두 저장 상태 saved/failed를 실제 수신한 뒤 정상 종료되면 완료된 결과를 그대로 유지한다. 이 close를 새 장애 안내나 재시도로 바꾸지 않는다. pending 상태의 close는 불확실 상태를 보존한다.

## 명령과 snapshot

메모리에만 최대16개의 원본 command_id/client_seq/action/known_revision을 순서대로 보존한다. 전송 전에 등록하고 효과/보드/통계는 낙관 반영하지 않는다. 끊김 시 ACK timer를 중단하고 후보 복구를 기다린다. 새 connection 첫 snapshot은 같은 match/rules와 감소하지 않은 revision·새 epoch를 확인하고 server_seq를 새 stream으로 시작한다. clientSeq는 서버 소비 cursor와 보존한 명령의 최대 순번 중 큰 값으로 설정한다. 기존 명령은 UUID/seq/action/known_revision을 유지하고 session_epoch만 바꿔 순서대로 재전송한다. 서버 cache가 원본 ACK를 반환하며 원본 ACK revision이 현재 view보다 작아도 정상이다. 미소비 명령에 대한 서버 거절도 권위 ACK로 소비하고 자체 성공을 추정하지 않는다.

재전송 명령은 ACK10초를 다시 갖고, 이미 재전송한 명령이 또 timeout이면 반복 복구로 무한 수명을 만들지 않고 불확실 종료 안내로 멈춘다. u32::MAX cursor에서는 새 입력을 만들지 않되 이미 존재하는 같은 UUID의 cached ACK 복구는 허용한다. 다른 탭이 소비한 cursor를 무시하고 새 순번을 되돌리지 않는다. terminal result/권한 변경/예산 소진/dispose는 모든 명령 timer와 메모리를 지운다. reload 후 미확인 UUID 복원은 보장하지 않고 일반 storage에 온라인 입력을 넣지 않는다. reload 새 입력 cursor는 #74를 따른다.

## 검증과 제한

시나리오 TS15/16/20/21/26/31/36의 TDD: 계획 delay/지터/동시1·정확30초/개별10초와 signal 무시 late connect, 연결 lease 교체/역순 완료·오래된 close, offline 후보와 fresh auth·1008/peer logout, 서버 cursor/16pending/u32 상한, ACK 유실 뒤 같은 UUID cached ACK/중복 효과0·known revision 유지, 늦은 옛 ACK, snapshot/rules/revision 오염, 복구 중 입력0/계정·탐색 종료와8언어를 확인한다. 실제 PostgreSQL/Rust WS/HTTPS PC/mobile에서 ACK만 유실시킨 실제 명령→재연결→cached ACK·flag1회/같은 판·시간 진행·새 입력과 SQL 결과, 실제 navigator offline→복구·서버 logout 거절을 검증한다. Playwright WS route는 실제 서버에 연결한 프레임 일부만 유실시키고 서버 권위를 흉내 내지 않는다.

영속 active journal/재시작 abort/인증 결과 조회와 상위18 통합 출구는 후속이다. 실제 OAuth42·장비26·사람27 검수는 별도다.
