# ADR0071 · 목적별 동의 포트와 설정 화면

- 상태: 채택 · 승인된 FR11/13 · NFR01/02/06/07/08 · #152, 상위 #21
- 선행: #20/PR146·#150/PR151 병합/CLOSED, develop9eb6a91
- 시나리오: TS19/24/26/29/31/36

## 범위와 권위

브라우저 ConsentPort를 CMP 구현과 UI 사이에 둔다. 현재 실행 앱은 실제 어댑터 없이 unavailable만 조립하며 광고/선택 분석·수집을 켜지 않는다. fake CmpPort는 테스트에만 주입한다. 실제 인증 CMP/TCF·지역/만료·다른 탭·계정/세션 scope/현재 revision, 광고 SDK와 서버 같은 transaction의 동의/권리/보존은 #21/#147의 필수 후속이다. 이 포트 view는 서버 저장 proof나 CMP 인증 증명이 아니다. 임의 client consent_changed가 server grant를 만들지 않는 기존 essential-only 경계를 유지한다.

CmpPort는 현재 검증된 최소 선택의 read, 선택 변경 subscribe, 명시 reopen(AbortSignal)을 제공한다. read는 raw vendor payload/TC string/계정/토큰 대신 닫힌 unknown/denied/error 또는 valid+각각 boolean analytics/advertising만 반환한다. ConsentPort는 허용 상태를 캐시하지 않고 매 read에서 현재 adapter snapshot을 엄격히 검사한다. 미설정·구독 실패·read 예외·형태/필드 불일치·unknown/denied/error는 두 목적 모두 차단한다. 분석 true는 광고 허용이 아니며 valid의 명시 true인 해당 목적만 허용한다. 후속 소비자는 비동기 경계/실제 요청 직전에 다시 read해야 하며 오래된 UI view를 권한으로 재사용하지 않는다.

## 재열기와 수명

재열기는 한 번만 진행하며 pending 중 두 목적은 차단한다. Promise 반환값을 동의로 해석하지 않고 완료 뒤 현재 read를 다시 사용한다. 실패는 차단/안내, 취소는 즉시 caller를 해제하고 late completion을 무시한다. AbortSignal을 어댑터로 전달하고 caller의 abort listener·진행 중 요청·구독은 완료/취소/dispose에서 정리한다. 실제 어댑터는 abort 후 자기 UI/작업도 정리해야 한다. 변경 callback은 이미 검증·갱신된 현재 snapshot을 알리는 신호이며 메시지/Promise payload 자체로 허용하지 않는다. 구독 설정이 실패하면 안전한 변경 관측이 불가능하므로 허용하지 않는다.

설정은 공통 ConsentSettings organism으로8언어 현재 상태·두 목적·명시 재열기/취소를 표시한다. 미설정에서는 재열기 버튼을 비활성화하며 가짜 선택 UI나 성공을 만들지 않는다. 각 화면의 기존 footer/설정 진입을 유지한다. 컴포넌트가 나가면 해당 명시 요청을 화면 제거와 같은 commit의 layout cleanup에서 즉시 취소하고 늦은 UI 갱신을 막는다. 일반 passive effect의 after-paint 정리를 기다리지 않는다. 취소 자체는 동의를 생성하지 않으며 이후 권한도 현재 검증된 adapter 선택으로만 판단한다. 동의 로딩/실패는 ActivityPort lock이나 로그인·게임·광고 요청을 만들지 않는다. 선택 동의를 브라우저 저장소에 복사하지 않으며 설정 고지 읽기/닫기는 동의가 아니다.

## 검증

fake port 단위에서 목적 독립·닫힌 상태/잘못된 값·예외·미설정/구독 오류, 매 조회 fresh snapshot·철회·재열기 pending/동시/실패/취소/dispose/late completion을 Red→Green으로 관측한다. 실제 DOM의 설정 재열기와 철회·다른 목적 상태,8언어 unavailable와 실행 앱의PC/mobile·저장소 차단/서버 실패·무광고/로컬 연습을 검사한다. 관련 full/문서/Mermaid·코드 리뷰·최신6checks/정확head 병합을 실제 기록한다. 실제 CMP·계정/탭·DB/광고 통합을 이 foundation으로 완료했다고 보고하지 않는다.
