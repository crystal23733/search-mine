# #56 · 공개 관측 온라인 봇 actor

#17/M3 · FR06·TS10/11/21 · [ADR0026](../adr/0026-public-observation-online-bot-driver.md) · 선행 #54 PR55/develop8d2d08e 병합·종료.

`online/bot.rs`의 BotAgent/Factory는 공개 core Projection·서버 시각만 받는다. CoreBotFactory가 Board/보드 seed를 받지 않고 독립 OS entropy로 행동 RNG를 만든다. profile·판단 간격·노게스/간파 추론은 Rust BotPolicy가 결정한다. 명시적인 create_with_bot은 하나의 None seat만 허용하며 인간 OAuth authority를 가장하지 않는다. 시스템 intent는 pure MatchState의 타입이고 도메인은 worker/전송을 알지 않는다.

각 driver는 mutable agent를 하나의 blocking job으로 이동하고 전역 permit은 그 실제 closure가 소유한다. oneshot 완료 slot은 입력 mailbox capacity와 분리된다. actor가 매 ingress 후 public revision/시각·100ms를 검증하고 같은 actor의 현재 core 규칙으로 적용한다. stale·종료 뒤 완료는 폐기하고 actual 작업이 진행 중인 동안 새 계산을 겹치지 않는다. bot의 open/accuse 이후 proof refresh와 public delta·결과 저장/정리는 기존 경로다. factory/planner 실패나 worker panic은 core ServerFailure로 종료한다.

최초6개 실제 actor 행동 시험은0passed/6failed Red(.tmp/bot56-red.log)였으며 구현 뒤 같은6개가 통과했다. 추가 age101ms·opponent private flag·첫 bot seat·factory 실패/panic 검사 포함10개Green(0.57s), locked all-target clippy·전체 server tests·문서108/Mermaid34 exit0이었다. panic fixture가 시험용 mutex를 잡은 채 panic해 teardown이 PoisonError인 실패는 guard를 먼저 반환하도록 고쳤다. 제품의 panic→ServerFailure 경로는 실제 시험으로 확인했다.

3난이도의 실제 core 봇은 인간에게 aggregate progress만 보내고 완료 결과의 bot account는 None이다. blocking bot 중 인간 입력 ack가 진행하며 public 상태가 바뀐 뒤 이전 intent는 적용되지 않는다. 상대의 private flag는 bot의 공개 revision을 바꾸지 않아 정당한 intent를 무효화하지 않는다. global1worker/두 매치·seat당 실제 한 작업과 종료 후 결과 단일 저장을 통제된 포트로 검증한다. pure state에서도 시스템 입력이 인간 seat를 차지하거나 지뢰 기절을 우회하지 못한다.

pm-ai-shipping:code-review 정확성·보안(scope56/base8d2d08e)으로 factory→public planner→oneshot→actor/state→projection/result의 완전한 흐름을 검토한다. source는 서버 public projection/내부 난이도·시각이며 authority는 core이다. secret 엔진/온라인 보드 seed가 planner signature나 public DTO에 들어가지 않는다. oneshot receiver는 해당 actor/seat의 소유여서 종료·재사용한 UUID에 과거 완료가 전달되지 않는다. 최종 coverage/필수 checks와 병합은 PR 실제 결과에 기록한다.

production main/Lobby 예약→registry 조립·인증 lobby/OAuth 초대/UI는 아직 후속 #17 작업이다. core/actor 시험을 미니 PC 성능26·실제 키42·사람 관찰27의 완료 근거로 사용하지 않는다.
