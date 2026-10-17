# ADR0073 · 동의의 현재 컨텍스트·만료·다른 탭 철회

- 상태: 채택 · 승인된 FR01/10/11/13 · NFR01/02/05/06/07/08 · #156, 상위 #21
- 선행: #20/PR146·#152/PR153·#154/PR155 병합/CLOSED, develop a77283c
- 시나리오: TS18/19/20/24/26/29/31/35/36

## 경계와 현재 선택

실제 CMP와 독립적인 브라우저 guard를 CmpPort와 ConsentPort 사이에 둔다. 실행 앱은 실제 어댑터 factory 없이 unavailable만 조립한다. 테스트 fake와 검증되지 않은 선택을 실행 앱에 넣거나 광고/선택 수집 API를 켜지 않는다. 인증 CMP/TCF·지역 정책·서버 저장 proof와 같은 transaction의 권리/동의 검증은 #21/#147 필수 후속이다.

추가 어댑터 factory는 ConsentContextPort를 받고 선택 검증을 시작할 때 capture한 메모리 token을 유지한다. 검증 결과는 닫힌 valid+analytics/advertising boolean+context token+양의 안전 정수 revision+validFrom/expiresAt(UTC ms)만 제공한다. token은 계정/credential을 담지 않는 메모리 참조이며 직렬화·재사용 가능한 서버 proof가 아니다. 늦은 결과에 새 token을 붙이지 않고 해당 작업을 시작한 token을 사용한다. SDK의 현재 선택을 새 컨텍스트에서 재검증한 결과만 새 token을 받을 수 있다.

guard는 매 요청 직전 read에서 현재 token 동일성·시간 범위(validFrom≤now<expiresAt)·revision을 다시 검사한다. 어댑터 read와 receipt 필드 접근 뒤 컨텍스트를 다시 확인하고 시각을 새로 읽어, 조회 중 컨텍스트 변경/만료도 같은 read에서 차단한다. 같은 컨텍스트의 낮은 revision, 같은 revision의 선택/기간 변조, 잘못된 값/추가 필드는 차단한다. unknown/denied/error·만료 뒤에는 이전 revision이 다시 허용되지 않는다. 새 revision의 현재 검증 결과만 복구할 수 있다. 거부는 valid의 두 목적 false로도 표현하며 두 목적은 독립이다. UI에 vendor payload/token/계정/revision을 노출하지 않는다.

최초 receipt의 남은 유효시간을 monotonic deadline으로도 고정한다. 같은 revision의 반복 조회로 deadline을 연장하지 않고, wall/monotonic 시계 역행·잘못된 시각은 해당 컨텍스트에서 차단한다. browser clock은 서버 권위가 아니므로 이 추가 차단을 서버 동의 검증으로 해석하지 않는다. 만료 타이머는 현재 read를 재검사하고 구독을 갱신한다. 긴 기간은 최대2^31-1ms 단위로 나누고 dispose·컨텍스트 변경·차단 시 정리한다.

## 계정·탭·재열기 수명

현재 AuthPort의 ready/working·계정 ID·공개 revision, online/visible을 확인한다. 계정/세션 전환·로그아웃·offline·숨김·BFCache 복원·다른 탭 invalidation에서 token을 교체한다. 동일 계정으로 돌아와도 예전 token은 복구하지 않는다. 안정된 동일 컨텍스트의 단순 auth 알림은 token을 교체하지 않는다. capture에서도 현재 auth를 검사해 알림 누락을 권한 재사용 근거로 삼지 않는다. 계정 식별자는 기존 메모리 상태 비교에만 쓰고 새로 저장·전송하지 않는다.

명시 재열기는 token을 먼저 교체해 이전 선택을 차단한다. 현재 context와 caller AbortSignal에 요청을 결합하고, 컨텍스트 변경·취소·dispose 시 adapter signal을 abort하며 adapter가 취소를 무시해도 caller는 해제한다. 늦은 완료/예전 receipt로 허용하지 않는다. provider Promise의 반환값은 선택이 아니다.

브라우저 채널 liar.consent.v1은 값 없는 invalidate 문자열만 전달한다. provider 선택 변경은 다른 탭에 이 신호만 보내며 수신 측은 현재 token을 교체한다. 수신 신호는 재방송하지 않으며 어떤 메시지도 grant가 아니다. 채널 생성/구독/전송 실패·미지원은 선택 목적 차단, 게임/로그인/로컬 연습은 계속한다. 선택·token·계정·credential은 localStorage/IndexedDB/채널에 복사하지 않는다. 어댑터 미설정에서는 불필요한 채널/계정 구독도 만들지 않는다.

## 검증

제어된 Clock/Context/CmpPort로 목적 독립·미래/만료 경계·revision 역순/동일 변조·시계 역행·timer/dispose·pending과 late completion을 행동 Red→Green으로 확인한다. 실제 DOM/BroadcastChannel 대체 포트·AuthPort에서 계정/peer/숨김/복원·실패와8언어 현재 화면 갱신을 검증한다. 실행 앱 PC/mobile 미설정 차단·WASM 연습·전체 회귀, docs/Mermaid와 최신6checks/정확head 리뷰·병합을 기록한다. 실제 provider/DB 통합·#83/#141·외부 검수 완료를 주장하지 않는다.

기술 근거: [HTML BroadcastChannel](https://html.spec.whatwg.org/multipage/web-messaging.html#broadcasting-to-other-browsing-contexts), [브라우저 타이머 최대 지연](https://developer.mozilla.org/en-US/docs/Web/API/Window/setTimeout#maximum_delay_value). 구현은 채널 메시지를 권한 전달로 사용하지 않는다.
