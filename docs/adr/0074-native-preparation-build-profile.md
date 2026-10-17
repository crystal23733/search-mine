# ADR0074 · 고정 native 준비 예산과 개발 코어 빌드

- 상태: 채택 · 승인된 NFR01/02/04/05/07/08 · #141
- 선행: #156 CLOSED·PR157/develop28be2ba 병합 확인
- 시나리오: TS15/16/20/21/27/31

## 확인한 유발 조건

#141은 실제 main의 캐시 준비8초·pair4초·admission2초 경계에서 간헐 실패했다. 과거 모든 실패의 seed/CPU 원인이 같다고 단정하지 않는다. 기존 공개 생성 벤치마크의 tail seed5108을 같은 실제 Rust 코어로 비교했을 때 unoptimized 개발 빌드는 생성8.650초(후보6개), release는1.371초, 코어만 opt-level1을 적용하고 개발 검사를 유지한 빌드는1.666초였다. 개발 빌드 비용만으로 기존 캐시 준비8초를 초과하는 구체 조건이다. 개발 PC의 단일 관측이며 미니 PC 성능목표 #26의 달성이 아니다.

## 결정

먼저 공개 tail seed의 실제 생성·인증을 기존 캐시 준비8초 안에 완료하는 행동 검사를 실행하여 Red를 확인한다. 워크스페이스 dev profile에서 liar-core만 opt-level1로 빌드한다. test profile의 dev 상속으로 실제 CLI/main·단위·PG/HTTPS·계측 검사가 같은 코어 설정을 사용한다. 다른 서버 코드는 개발용 디버그 정보를 유지하고 debug assertion·overflow 검사도 끄지 않는다. release/WASM 배포 설정·코어 알고리즘/RNG/규칙/예산·OsSeedSource와 실제 main 조립은 유지한다. 의존성이나 버전은 바꾸지 않는다.

캐시 준비8초·pair4초·admission2초·SQL1초·실제 서버 준비 관측을 유지한다. fake 판이나 고정 seed를 실제 서버에 주입하지 않고, 공개 seed는 경계 회귀 테스트에서만 사용한다. 시험의 timeout/retry·브라우저 계측 설정을 변경하지 않는다.

## 검증과 판단 범위

공개 tail의 Red→Green·실제 생성 certificate·native/WASM 결정론과 전체 단위 검사를 확인한다. 실제 PG/계측·main 재시작/취소/owner-loss 반복·HTTPS/WASM/WS 전체·문서/Mermaid·코드 리뷰·최신6checks/정확head를 기록한다. 최적화된 계측 coverage도 실제 threshold를 통과해야 하며 미측정 줄을 성공으로 추정하지 않는다. 최초 실패/진단은 별도 보존한다. 새 실패가 있으면 해당 단계 자료로 계속 조사하며 성공 반복만으로 과거 모든 #141 원인을 입증했다고 하지 않는다. #83 native lifecycle과 외부42/147·실장비26/사람27은 별도다.

근거: [공개 tail 측정](../verification/05-generation-solver.md), [Cargo profile 상속·package override](https://doc.rust-lang.org/cargo/reference/profiles.html).
