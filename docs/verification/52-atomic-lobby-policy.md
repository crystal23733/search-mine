# #52 · 원자 큐·친구방 정책

#17/M3 · FR06/07·TS10/12 · [ADR0024](../adr/0024-atomic-lobby-and-online-composition.md) · 선행 #16 PR51/develop951dff11 병합·종료.

`crates/server/src/lobby/policy.rs`는 IO 없는 정책이다. FIFO 큐는 기한10000ms 전에 도착한 인간을 예약하고 기한 이상이면 원래 난이도의 bot을 한 번 예약한다. 방은 canonical8문자/10분·2seat·둘의 ready를 요구한다. account는 큐/방/준비 중 한 위치만 가지며 모두 capacity에 포함한다. code는 초대이며 계정 인증을 대신하지 않는다.

비동기 준비의 identity는 entity UUID+단조 generation이다.5초 준비 만료·방 원래 만료·취소·unready 뒤 old completion은 commit하지 못한다. host leave는 남은 seat를 승격하고 ready를 초기화한다. 큐 준비 취소는 남은 사람을 원래 FIFO/기한으로 돌려보내고 기존 대기자와 즉시 재예약한다. source의 원본 ticket ID 모두를 live collision 검사에 포함한다.

최초9개 행동 시험은 정상 큐/방 전이를 하지 않는 stub에서0passed/9failed의 실제Red였다. 구현 뒤9개Green을 확인했다. 후속 리뷰의 기존 대기자 재예약 누락과 둘째 ticket ID 충돌 누락은9passed/2failed Red였고 수정 뒤11개Green이다. fmt·locked all-target clippy·문서104/Mermaid34도 실제 통과했다. lobby 도메인95% gate를 추가했고 실제 CI 측정은 PR에서 확인한다.

검토 범위는 develop951dff11 대비 정책/수명/미완료 비동기 correlation과 승인된 계약이다. 외부 입력 UUID/nil/code/clock/overflow/capacity, queue/room 교차 점유, 만료 equality와2회 commit, ready false→true의 같은 entity·다른 generation을 강제 검사한다. 서버 UUID/코드 생성·전송·인증·보드 공급은 port의 후속 책임으로 유지한다.

추가 검수는 원래 방 만료1ms 전 준비·같은 code/entity 재생성 뒤 old completion, 준비 중 guest leave의 host 보존, waiting/room/preparing의 capacity와 중복 계산을 검사한다. 실제14개 결과와 최종 coverage는 PR에서 확인한다.

이 구현만으로 실제 온라인 매칭/공개 관측 봇/HTTP·WS/친구 화면/OAuth 초대 왕복을 완료했다고 보고하지 않는다. 후속 하위 이슈와 통합 출구가 모두 완료된 뒤 상위 #17을 닫는다. 실제 OAuth42·홈서버 성능26·사람 관찰27은 해당 실제 입력/환경에서 검증한다.
