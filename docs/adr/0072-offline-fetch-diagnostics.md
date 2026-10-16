# ADR0072 · 오프라인 업데이트의 fetch 응답 수명 진단

- 상태: 채택 · 승인된 FR10 · NFR01/02/05/07/08 · #154, 상위 #83
- 선행: #85/PR86·#152/PR153 병합/CLOSED, develop a203b5a
- 시나리오: TS18/20/26

## 관측 빈틈과 결정

#152 최신 CI의 두 탭 업데이트 최초 실패에서 waiting installed·양쪽 boot1과 skipWaiting 미완료를 보존했다. 기존 JS waitUntil pending과 native worker stop/start만으로는 fetch 응답 Promise나 브라우저 응답 처리의 잔여 작업을 판단할 수 없다. 원인은 미확정이며 반복 성공이나 재시도를 해결 증거로 사용하지 않는다.

loopback fixture가 production worker 뒤에 붙이는 producer에 fetch event ID·respondWith Promise와 FetchEvent.handled의 각각 pending/fulfilled/rejected를 기록한다. ID는 worker boot 안의 순번이며 bootAt(timeOrigin)으로 같은 revision의 재시작을 구분한다. 원시 URL/query/header/cookie/body/client 계정은 기록하지 않는다. 요청의 닫힌 method/mode/destination과 worker revision·단조 경과 시간만 추가한다. 기존 lifecycle 로그에도 두 pending counter를 포함해 skipWaiting 시점과 비교한다.

원본 respondWith/waitUntil을 같은 this·같은 인자로 먼저 호출한다. 반환값과 동기 예외를 보존하고, 성공한 호출의 원래 native Promise에 관측 callback만 붙인다. Promise.resolve로 임의 thenable을 다시 실행하지 않으며 직접 Response/관측 불가 입력은 unavailable로 표시한다. 동기 거절은 pending을 증가시키지 않는다. fetch event의 handled는 별도로 관측하며 Promise가 없는 환경은 unavailable만 기록한다. body를 읽거나 clone하지 않고, 새 waitUntil·요청·타이머·debugger 연결·worker 강제 활성화를 추가하지 않는다. 제품 SW·coordinator·activity·제한시간·retry·기본 네트워크 계측 설정은 유지한다.

응답 Promise fulfilled는 유효 Response 채택이나 body 전송 완료 증명이 아니다. handled fulfilled도 body 소비 완료/native idle 전체와 같지 않다. worker 종료·재시작으로 사라진 JS counter는 boot 경계와 함께 해석한다. 추가 관측 자체의 scheduling 영향은 실제 브라우저 반복과 CI 원자료로 검토하며 성공으로 원인을 확정하지 않는다.

## 검증과 추적

Node VM에서 실제 fixture 원문을 실행해 제어된 pending/거절/동기 예외·두 요청 역순 완료·독립 waitUntil·같은 Promise/반환값/예외를 검증한다. 실제 ReadableStream Response를 관측 후 원래 소비자가 끝까지 읽어 body 미소비·미잠금을 확인한다. 로그의 닫힌 필드·비밀 문자열 부재와 단조 시간을 검사한다. 이 검사는 전체 check/CI에 포함한다.

기존 production WASM PC/mobile 두 탭 검사를 원래10초·재시도0으로 반복하고 전체 브라우저·docs/Mermaid·필수6checks를 실행한다. 산출물의 producer 응답/handled 수명을 실제 확인하고 실패 자료도 보존한다. #154의 진단 검증·병합과 #83의 제품 원인 해결을 구분한다.

기술 근거: [W3C respondWith](https://w3c.github.io/ServiceWorker/#fetch-event-respondwith), [FetchEvent.handled](https://w3c.github.io/ServiceWorker/#fetch-event-handled), [waitUntil](https://w3c.github.io/ServiceWorker/#extendable-event-waituntil).
