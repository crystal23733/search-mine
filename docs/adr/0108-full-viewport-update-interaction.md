# ADR0108: 업데이트 입력 장벽의 실제 viewport 피복

상태: #235에서 대조 검증 중. FR10/NFR01/02/05/06/07/08, ADR0102의 동기 입력 차단 범위를 보완하는 구현 후보다. 게임 규칙·사용자 데이터·기존 deadline/retry를 바꾸지 않는다.

## 관측과 후보

PR234의 실제 모바일 trace는 두 ACK 이후 Warm Up1과 이전 worker의 stopping→재시작을 기록했다. 기존 중앙 modal은 문서가 inert이지만 dialog의 실제 hit-test 사각형은 header/link 좌표를 덮지 않는다. Chromium의 speculative warmup은 DOM click handler 이전 경계도 가지므로 “클릭 이벤트가 안 전달됨”과 “실제 모든 좌표가 장벽 안임”을 구분한다.

업데이트 modal의 외곽을 고정된 전체 viewport hit-test 영역으로 만들고 상태 panel은 그 안에서 중앙 배치한다. prepare ACK 전 showModal 순서, 이름/포커스/ESC 차단·실패NACK·15초 lease/해제 복원과 게임 busy 거절은 유지한다. 임의 callback sleep·warmup flag·worker 강제 stop/start·기존 timeout 완화는 도입하지 않는다.

## 인수와 해석

실제 Chromium PC/mobile·8언어에서 header와 네 모서리 좌표가 dialog 사각형에 포함되고, showModal 이전 ACK가 없으며 원래 focus가 복원됨을 확인한다. 기존 두탭/게임busy/캐시삭제와 연속입력을 retry0로 반복하고 완전 native trace의 Warm Up/이전 worker 재시작을 별도로 기록한다. 후보가 통과하더라도 Chromium 자체를 수정했다고 하지 않는다. 실패하면 결과를 보존하고 추가 원인을 조사한다.

[기존 관측과 한계](0102-update-interaction-barrier.md), [검증235](../verification/235-public-launch-wiring.md), [Chromium exact revision features](https://raw.githubusercontent.com/chromium/chromium/971a7443b0c9b0a9b2860529b33331b76077ec62/third_party/blink/common/features.cc)를 근거로 삼는다. 이 후보는 동의 관리 설계 변경과 별개의 입력 안전성 검증이다.
