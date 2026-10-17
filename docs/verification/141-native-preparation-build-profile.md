# #141 · native 준비 예산과 코어 개발 빌드

NFR01/02/04/05/07/08 · TS15/16/20/21/27/31 · [ADR0074](../adr/0074-native-preparation-build-profile.md).

선행 #156/PR157 병합·CLOSED와 develop28be2ba clean/원격 일치를 확인했다. 코드 변경 전에 ADR·시나리오·추적표를 갱신했다. 기존 공개 tail seed5108은 실제 코어의 unoptimized8.650초/release1.371초/core opt1 개발1.666초로 후보6개를 생성했다. 임시 비교는 `.tmp/probe141`에 보존하고 실제 온라인 seed를 기록하지 않았다.

공개 tail 실제 생성의 통합 테스트는 기존 unoptimized 빌드에서0pass/1fail·8.20초·Elapsed로 Red였다. liar-core dev opt1 적용 뒤 동일 검사와 기존 pool 취소/상한/재충전/역순/entropy 동작을 포함한12개가1.83초에 통과했다. certificate를 실제 검증하고 안전 칸216개를 확인했다. 실제 main·migration·HTTPS fixture 바이너리도 같은 설정으로 빌드했다.

전체·실DB/계측·main 반복·코드 리뷰/CI 결과는 실행 뒤 기록한다. 준비8초·pair4초·admission2초·SQL1초·OsSeedSource를 유지하며 실제 서버에 성공 판/seed를 주입하지 않는다. 개발 PC 측정과 미니 PC 성능 목표를 구분한다. 과거 모든 실패 원인의 동일성과 #83 원인 해결을 주장하지 않는다.
