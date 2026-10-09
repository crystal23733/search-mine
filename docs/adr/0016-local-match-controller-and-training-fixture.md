# ADR0016: 로컬 대전 controller와 작은 학습용 실제 규칙 판

- 날짜: 2026-10-07
- 상태: 채택 — #12, FR06/08, TS11/13/36. PR38/#11 병합·종료 확인.

## 결정

로컬 연습은 PracticeCore worker와 실제16×16 Rust RuleEngine/3난이도 공개 관측 BotPolicy를 연결한다. controller는 Clock/seed source/core port를 주입하고 단일 FIFO로 tick/input을 처리한다. 입력 serial은 init 뒤1부터 증가하며 같은 input을 재전송할 때 ID/seq를 유지한다. pending tick은 합쳐 Worker64개 한도를 넘기지 않고 종료/unmount는 타이머·worker를 정리한다. UI는 승인된 public view/ack만 반영하며 셀/공격 성공·승패를 추정하지 않는다. 앱 언어/설정 변경은 세션을 재생성하지 않는다.

practice는 로컬 seed를 crypto.getRandomValues로 만들고 route는 난이도만 유지한다. online seed와 구분하며 로컬 계정/개인정보/서버 요청을 만들지 않는다. MatchLayout은 모바일 단일 보드·PC 보드/요약 분할, 상대 봇 표시/공개 진행만, 시간·내 safe/gauge·공격·기절·실수/지목/결과·재시작/홈을 제공한다. 공통 BoardRenderer와 BoardOrganism을 재사용하고 게임/Pixi/Worker를 지연 로드한다.

학습은 `config/tutorial.toml`의3×3/2mine 공개 예제·게이지1로 짧게 진행한다. 일반 대전의16×16/게이지20과 분명히 구분해 안내하고 동일 RuleEngine의 safe gain·자동 공격 인증·correct accuse·reflection2초를 사용한다. Rust가 bundled 규칙에 학습 설정을 적용해 hash snapshot을 만들며 TS가 규칙을 다시 계산하지 않는다. 원천은 학습 전용 설정이고 온라인 매치에서 사용하지 않는다.

zero 오프닝 뒤 상대가C3을 미리 연 상태에서 학습자가C3 열기→게이지1→공격1회, 상대 자동 공격→C1 열기→지목1회를 수행한다. 고정 예제C2/B3의 두 지뢰는 C3의 공개 숫자2에서 증명되며 C1의 올바른 숫자1을 공개 이웃으로 설명한다. 힌트는 이 공개 학습 예제의 정해진 동작이고 online target/secret DTO가 아니다. Rust TutorialSession이 예상 단계와 중복 command를 검증하고 실제 ack/공개 view에서 단계를 전진한다. complete는 학습 완료이며 match 완료/광고 빈도에 넣지 않는다. typed TutorialView/Step은 Rust→생성 TS, core/worker는 별도 tutorial mode로 분리한다.

최초 방문은 저장된 학습 완료/skip 값이 없으면 tutorial로 이동한다. 명시적 tutorial route는 재학습한다. locale·초대 code/query/hash·유효 내부 return route를 유지하고 외부 redirect는 허용하지 않는다. skip도 다음 방문 자동 시작을 멈추지만 완료로 기록하지 않는다. 저장소 실패는 메모리 fallback·유실 안내이며 게임/skip은 계속된다. 새 탭에서 유실되는 경우를 숨기지 않는다. tutorial에는 광고 요청/표시0이다.

단위에서 FIFO·clock/init race·중복·세션 해제·저장소 failure, Rust에서 실제 공격/지목/reflection/duplicate와 잘못된 단계, 실제 browser에서 WASM+3난이도·학습/skip/replay·invite·언어 세션 보존을 검증한다. 30초 이해와 재미는 실제 참여자 E1/E2(#27) 관찰이며 자동 테스트로 대신하지 않는다. M2의 해당 수동 출구 조건은 미확인으로 유지하며 독립적인 이후 코드를 계속한다.

학습 순서와 다른 동작은 현재 공개 view와 rejected ack로 안내하고 세션을 종료하지 않는다. 어댑터는 학습 순서에서 거절된 input ID/seq/action과 ack를 최대8192개 보관해 단계 변경 뒤 같은 input이 갑자기 적용되지 않게 한다. conflict는 거절하고 monotonic 시간 검증은 Rust 세션이 담당한다. malformed/시간/내부 실패는 기존 안정 오류 계약을 따른다. 모바일은 학습 안내→보드→진행 요약 순서로, PC는 보드 좌측·안내/요약 우측으로 배치한다.

#68 전체 검증의 mobile trace에서256셀 관측이1.35초 걸렸고 완료 snapshot에는 실제 Starting in3s가 있었지만 다음 assertion이 시작하는 동안 카운트가 바뀌었다. E2E는 다른 난이도/재시작과 동일하게 남은1/2/3초의 실제 countdown 표시를 관측하고, 이후 공개 봇 진행·언어 변경의 동일 진행·재시작/정리를 확인한다. 전체3초 규칙/Clock origin은 Rust와 controller의 기존 계약대로 유지하며 제품 코드·timeout·retry를 바꾸지 않는다. 관측 수정은 PC/mobile 반복과 전체 browser에서 확인하고 실패 trace를 보존한다.
