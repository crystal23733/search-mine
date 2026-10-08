# ADR0026 · 공개 관측 온라인 봇 driver

2026-10-09 · #56/#17/M3 · FR06·TS10/11/21 · 선행 #54 PR55/develop8d2d08e 병합·종료. [봇 설계](../design/05-algorithms.md)의 Rust BotPolicy를 실제 online actor에 연결한다.

BotAgent/Factory는 공개 Projection과 서버 시각만 입력받는다. production CoreBotFactory는 OS entropy로 독립 행동 RNG를 만들며 Board/온라인 보드 seed/result metadata를 받지 않는다. Easy/Normal/Hard의 간격과 solver budget은 core가 결정한다. bot을 연결하는 create_with_bot은 정확히 하나의 None seat를 요구하며 인간 seat/서비스 authority를 bot 입력에 사용하지 않는다. 기존 human actor 생성 API는 유지한다.

BotExecutor의 전역 worker 수1~64와 driver당 한 실제 작업을 제한한다. mutable agent는 blocking closure로 이동하고 그 closure가 permit을 끝까지 소유한다. 완료는 bounded 입력 mailbox와 별도인 단일 oneshot slot으로 반환한다. actor는 매 입력 처리 뒤 완료를 관찰하며 같은 actor/server 시각에서 core Command를 적용한다. 봇의 공개 revision과 시작 시각을 캡처하고 달라진 revision·100ms 초과·역행·종료 뒤 intent는 폐기한다. agent는 계산 결과와 함께 돌아오며 계산 도중 timeout으로 새 worker를 중첩하지 않는다.

스케줄 probe는 최대20ms마다 가능하고 같은 서버 시각에서 다시 계산하지 않는다. countdown/기절/종료 중에는 새 계산을 만들지 않으며 지연된 결과도 적용 시 core의 현재 규칙이 다시 검증한다. 봇의 open/accuse 이후 proof refresh·공개 delta·결과 저장/정리 경로는 기존 actor를 공유한다. factory/planner 실패·panic 또는 내부 seq 포화는 core ServerFailure로 fail closed 종료한다. Drop/종료는 이미 시작한 CPU를 중지했다고 가정하지 않고 그 결과를 버린다.

실제 core 봇의 공개 진행·결과·3난이도와 controlled blocking port의 인간 입력 응답·전역/seat worker 상한·stale/종료 뒤 완료·factory/planner 실패를 Red→Green으로 검증한다. 같은 공개 view/다른 hidden Board의 동일 의도는 기존 core TS11 검사와 서버 port의 입력 제한을 함께 검토한다. 실제 Lobby reservation→registry/main 구성·인증 전송/초대 OAuth/UI는 후속 #17 하위 작업이다. driver 검사를 mini PC 성능이나 실제 외부 키42 검수로 보고하지 않는다.
