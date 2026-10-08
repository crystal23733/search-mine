# ADR0025 · 비공개 검증 보드 풀

2026-10-09 · #54/#17/M3 · FR02/06/07·TS02/10/11/12/21 · 선행 #52 PR53/develop16b7495d 병합·종료. [시스템 구조](../design/01-architecture.md)의 미리 검증한 보드 풀을 구현한다.

BoardSource는 제한된 대기 뒤 서버 전용 PreparedBoard를 반환한다. seed의 난수는 별도 SeedSource port이며 production은 OS cryptographic entropy를 사용한다. Rust RulesSnapshot과 BoardGenerator/솔버를 재사용하고16×16/40의 노게스 인증을 통과한 판만 저장한다. Board/seed는 Serialize/Debug·공개 프로토콜·client storage에 넣지 않는다. 봇 행동 RNG도 보드 seed에서 파생하지 않는다.

pool의 cached+generating 총 수를 slot semaphore와 bounded channel로 제한한다. 생성 worker 수는 별도 상한을 가지며 각 생성은 core의 candidate/solver budget을 넘지 않는다. slot은 실제 blocking closure 안에서 소유하고 준비된 cache까지 유지하며 소비 시 반환한다. 시작한 blocking 작업의 async abort가 CPU를 중지한다고 가정하지 않는다. Drop은 새 작업/송신/slot 대기를 중지하고 이미 시작한 blocking 작업은 예산 안에서 끝나며 permit을 반환한다. 생성 실패는 제한 간격으로 재시도하며 empty pool은 bounded take 이후 unavailable이다.

설정 범위는 pool1~256·workers1~64(≤pool), take0 초과~2초다. 개발 값은 출시 capacity/성능이 아니다. pool은 게임 엔진의 countdown을 미리 시작하지 않는다. 후속 조립은 보드 소비 후 RuleEngine을 같은 rules와 server 시각으로 생성하고, 살아 있는 Lobby reservation/entity+generation을 같은 lock/시각에서 확인하여 MatchRegistry 생성·commit한다. 취소된 보드를 다른 요청의 완료로 잘못 연결하지 않는다.

#54는 실제 기본 보드의 인증/고정 zero와 entropy·재충전·슬롯/worker bounds·대기 실패·Drop 중 실제 blocking 수명을 검증한다. 후속 #17 하위 작업이 runtime 구성/온라인 관측 봇/인증 전송/초대 OAuth·화면을 연결한다. 미니 PC20ms·동접·실제 키42·사람 관찰은 이 검사로 주장하지 않는다.
