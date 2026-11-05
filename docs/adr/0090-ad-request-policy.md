# ADR0090 · Rust 광고 요청 정책과 예약 계획

- 상태: 채택 · #194 / 상위22 · FR12/13·NFR01/02/05/07/08
- 선행: #190/PR191·#192/PR193 MERGED/CLOSED / develop9e88bf4
- 시나리오: TS23/24/25

## 독립 구현 범위

사용자의 순차 진행 위임에 따라 승인된 설계11의 빈도·화면 규칙을 Rust의 IO 없는 광고 정책으로 구현한다. 실제 CMP·SDK·완료 집계/저장 adapter가 없는 실행 앱은 계속 광고0이다. 이를 상위22 전체 완료나 실제 다중 탭 원자성을 구현한 것으로 보고하지 않는다.

AdOpportunity는 폐쇄된 placement/menu·result, 현재 화면, online·advertising_allowed·sdk_ready·exclusive_slot의 관측값을 받는다. 하나라도 불충족이면 차단하며 countdown/playing/tutorial/other는 광고 화면이 아니다. menu 배너 계획은 menu 화면에서만, result 전면 계획은 result 화면에서만 반환한다. 입력은 adapter가 매 요청·늦은 callback 직전 새로 확인해야 하며 허용 bool을 권한 증거로 저장하거나 외부 API에서 신뢰하지 않는다.

결과는 정상 match/daily 완료만 허용한다. tutorial/abort·이미 예약된 결과를 차단한다. 카운터는 온라인/현재광고목적 등 설계11의 적격 조건으로 집계된 값이어야 하며 이 순수 정책은 완료 기록을 수집하지 않는다. 기기의 완료 번호와 마지막 요청 번호를 보유하고 로그인 계정은 서버 카운터도 필수다. 무계정은 기기만 사용한다. 번호는 u32 범위에서 마지막 요청은3이상·완료이하를 검증하며 첫 요청은 완료≥3, 이후 완료−마지막 요청≥3이어야 한다. 양쪽 모두 만족해야 하므로 값 중 단순 max를 취해 제한을 완화하지 않는다. counter 이상/누락은 차단한다.

ReserveResult 계획은 기기/계정의 마지막 요청 번호를 각각 현재 완료 번호로 바꾼 다음 상태를 반환한다. 잠금 adapter는 이 계획과 판별 예약표시를 SDK 호출 **전에** 원자적으로 보존해야 하며 실패하면 광고를 생략한다. no-fill/실패/취소 뒤 복구하는 정책 API는 만들지 않는다. 요청과 실제 impression을 구분한다. menu 계획은 결과 빈도를 소비하지 않는다.

## 경계와 검증

Rust core가 유일한 빈도/화면 규칙 원천이고 protocol은 생성 TS와 최대2048byte JSON decoder를 제공한다. 알 수 없는 필드/enum·중복 필드·잘못된 수·잘못된 version을 거절한다. WASM은 같은 decoder/정책 함수를 호출하며 seed/게임 상태/계정 식별자·동의 proof는 받거나 내보내지 않는다.

시나리오→행동 Red→Green으로 1/2/3/4/5/6·실패 후 slot 유지·양쪽 제한·중복 결과/slot 경쟁·화면/동의·invalid counter를 native에서 검증하고 실제 브라우저 WASM과 native 생성 fixture를 비교한다. 의미 있는 property로 임의 유효 카운터에서 요청 간격과 예약 뒤 재요청 차단을 확인한다. 문서/생성물/전체회귀·coverage·정확 head/코드 리뷰 뒤 병합한다.

실제 서버 완료 집계·계정/기기 저장의 CAS·다중 탭 잠금·게임 화면 hook/배너 제거 확인·callback 취소·대기 timeout와 노출 종료 barrier·SDK/CSP·선택 이벤트 producer는 #22 후속이다. 특히 노출 중 임의 timeout으로 countdown을 시작하지 않는다. 실제 CMP·운영 계정/도메인은 #147/21이다. 이 단계는 공개 광고 API·SDK 호출·추적/저장을 추가하지 않는다.
