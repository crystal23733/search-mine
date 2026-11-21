# #244 다인전 방 조사·설계 보강

대응 FR01~18/NFR01~08·TS37~45. [설계20](../design/20-multiplayer-rooms.md)·[ADR0110](../adr/0110-multiplayer-rooms-and-rankings.md)·[공식 조사](../research/2026-11-21-multiplayer-room-flows.md)·[사용자 선택](../workflow/approval-record.md)을 연결한다.

## 검토 내용

현재 RoomPolicy/Protocol/FriendsPanel의 두 자리·ready·8문자 초대, RuleEngine의 두 Player/Seat.other와 한 상대 projection, online actor/result/journal의2인 배열을 대조했다. 공개 방 목록과 같은 방 재대결은 현재 구현으로 오인하지 않는다.

사용자가 초기2~4인, 기존 레이스·첫 완주 후 즉시 종료·남은 안전칸 수 순위·무작위 상대 공격을 명시했다. 조사한 게임의 공개 메뉴/개발자 안내와 우리 판단을 나누고, 다른 지뢰찾기의 다인 대전 존재를 확인해 차별화/사용자 유입을 보장하지 않는다.

설계는 방/round 분리·명시 참가/ready/host start·다음 판 대기·revision/authority·같은 방 반복, 단일 코어/합계 두 lie/원래 source 반사, 공동 순위/DNF/동시 이탈·장애, N인 저장/권리·서버 용량·8언어/PWA/광고0·기존1:1 호환을 명세했다.

## 검증과 한계

로컬 문서 검사에서 Markdown 291개, 상세 설계 20개, 시나리오 추적 45개가 통과했다. Mermaid 39개와 diff 검사도 통과했다. 원격 검사와 병합 결과는 PR #250에 기록한다. 제품 코드·migration·Rust/웹/실DB/WS/Playwright·실제 타인 방 참가/대전은 이 설계 작업에서 실행하지 않았다.2~4인 공정성/성능/재미와 UI 구현 성공을 주장하지 않는다. 제품은 #245~249에서 실제 Red/Green·통합 검증을 수행한다.
