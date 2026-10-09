# ADR0035: 재접속 snapshot의 본인 입력 순번 복구

- 상태: 채택 — 승인된 #18의 재접속 계약 구현
- 날짜: 2026-10-09
- 선행: #17/#72·PR73/develop1b619ec2 병합·종료; 구현 #74

## 결정

새 epoch는 기존 매치·seat의 입력 순번이나 command cache를 초기화하지 않는다. 고정 cookie/Origin WS에 다시 연결하면 서버가 배정된 매치의 권위 snapshot을 먼저 보낸다. 별도 resume 입력·브라우저가 주장하는 last_server_seq/seat/epoch로 연결을 선택하지 않는다. snapshot의 session_epoch와 본인 last_client_seq(u32)를 함께 사용한다.

Rust RuleEngine의 읽기 전용 순번 조회가 유일한 원천이다. 캐시된 거절도 실제 소비한 순번이며 잘못된 epoch/sequence/conflict와 core 진입 전 future known_revision 거절은 순번을 소비하지 않는다. 매치에 재연결해도 원래 순번을 유지한다. server/protocol은 본인 순번만 공개하며 상대 cursor·내부 global revision·계정·seed·정답은 노출하지 않는다. core의 내부u64가u32를 초과한 경우에는u32::MAX로 투영해 새 입력을 안전하게 중단한다.

새 controller의 첫 snapshot이 본인의 clientSeq를 복구한다. 다음 입력은 복구한 값+1이며u32::MAX에서는 capacity로 입력을 중단한다. 입력 필드와 게임 규칙은 바꾸지 않고 TS에 sequence 소비 규칙을 복제하지 않는다. public stream의 첫 server_seq는 기존 actor 수명에서 이어질 수 있으며 이후에는 연속성을 검사한다. 현재 규칙 snapshot/hash와 공개 view revision 검증도 유지한다.

같은 command_id/client_seq/action을 새 session_epoch로 재전송하면 Rust core가 원본 cached ACK를 반환한다. ACK revision은 원래 공개 revision을 유지하며 flag·gauge·통계를 다시 적용하지 않는다. 이전 epoch와 바뀐 action/seq의 같은 ID는 거절한다. 이 순번 필드는 자동 재전송을 실행하는 기능이 아니며, 다음 하위 작업의 메모리 미확인 명령 큐는 예약한 순번까지 포함해 별도 검증한다. 새 문서로 기존 캐시된 웹이 새 snapshot을 수락했다고 주장하지 않는다. 아직 공개 배포 전인v1의 서버·생성 TS·strict decoder·웹을 함께 갱신하며 구 schema는 fail closed 한다.

## 검증과 후속

실제 TCP WS의 입력→disconnect/new epoch snapshot→cached retry→새 입력과 원본 revision/본인 cursor를 검증한다. 거절 소비와 잘못된 입력의 미소비, 상대 cursor 격리·u32 exhaustion을 강제 확인한다. 실제 HTTPS PC/mobile에서 진행 중 flag 입력→reload→같은 판/flag 유지→새 입력 적용을 확인하고 온라인 secret/입력을 일반 저장소에 기록하지 않는다.

자동 backoff·미확인 명령의 새 epoch 재전송·server grace 표시, 영속 active match journal·재시작 abort/저장 복구는 상위18의 후속 순차 작업이다. 로컬 시간으로 forfeit/abandon/abort를 확정하지 않으며 이 snapshot 계약만으로 상위18을 종료하지 않는다.
