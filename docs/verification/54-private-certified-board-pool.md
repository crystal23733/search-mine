# #54 · 비공개 검증 보드 풀

#17/M3 · FR02/06/07·TS02/10/11/12/21 · [ADR0025](../adr/0025-bounded-private-certified-board-pool.md) · 선행 #52 PR53/develop16b7495d 병합·종료.

`crates/server/src/lobby/board_pool.rs`는 SeedSource와 BoardSource port의 adapter다. OS entropy를 얻고 Rust BoardGenerator의 candidate/solver budget으로 인증한 판만 보관한다. PreparedBoard는 private Board·같은 rules snapshot·그 판의8byte seed를 함께 소유하며 Serialize/Debug를 구현하지 않는다. 게임 엔진이나 countdown을 미리 시작하지 않는다.

총 slot은 cached+generating을 포함하며 생성 worker 수와 별도로 제한한다. permit은 실제 blocking closure→캐시 항목에 귀속되고 소비·실패·폐기 때 반환된다. Drop은 생산 루프와 slot 대기를 멈추며 이미 admission된 blocking 작업의 CPU를 abort했다고 주장하지 않는다. 오류는100ms 간격 뒤 재시도하고 take는 lock 대기를 포함하여 최대2초로 제한한다. 취소된 take는 receiver lock을 반환하고 미래 보드를 소비하지 않는다.

최초7개 행동 테스트는0passed/7failed의 실제Red였다(.tmp/board54-red.log). 구현 뒤 같은7개가 통과했다. 추가 검수는 receiver 대기 취소·worker 첫 poll 전 Drop·지속 entropy 실패·생성 완료 순서 반전이다.16×16/40의 고정 opening zero·실제 노게스 재인증·seed 재생, cached+generating total·blocking worker 제한·소비 재충전·Drop 중 실제 작업 수명·잘못된 limits/snapshot/runtime/deadline을 검사한다. 실제 최종 개수/CI/coverage는 PR 결과로 기록한다.

리뷰 범위는 develop16b7495d 대비 entropy→bounded blocking 생성→cache→소비/Drop 흐름이다. caller의 snapshot과 생성 spec/seed를 다른 값으로 만드는 tamper·순서 반전 반례를 강제했고, 실제 blocking 작업이 진행 중인 동안 slot과 worker가 조기 반환되지 않는지 확인했다. pool은 UUID/reservation을 소유하지 않아 특정 대전과의 correlation은 후속 조립의 책임이다. 보드 seed를 봇 행동 RNG로 재사용하지 않는다.

아직 production 매칭·공개 관측 봇 driver·인증 lobby 전송·OAuth 초대 왕복·화면을 연결하지 않았다. source port 검사만으로 #17을 닫지 않으며 실제 외부 키42·미니 PC 성능26·사람 관찰27을 완료로 보고하지 않는다.
