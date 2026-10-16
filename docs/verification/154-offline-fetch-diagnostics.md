# #154 · 오프라인 fetch 수명 진단 검증

FR10 · NFR01/02/05/07/08 · TS18/20/26 · [ADR0072](../adr/0072-offline-fetch-diagnostics.md). 선행 #85/PR86·#152/PR153 병합/CLOSED 확인. 상위 #83은 원인 미확정 OPEN이다.

## 검증 계획

fixture 원문을 실행하는 제어된 Promise/stream 행동 Red→Green, 원본 반환/예외/Promise 보존·fetch 응답/handled/waitUntil 독립 수명과 민감값 미기록을 확인한다. PC/mobile 실제 WASM 두 탭 반복은 원래10초·retries0이며 전체 check와 CI에서 결과를 구분해 기록한다. 제품 SW·타이머·retry 완화는 없다. 아직 미실행인 검사를 성공으로 보고하지 않는다.

## 실제 행동 검증과 최초 실패

Node VM 최초6개는0/6 Red→6/6 Green이었다. 추가 리뷰에서 thenable의 중복 실행을6pass/1fail(2회 실행)로 확인하고 Promise.prototype.then의 native Promise 관측으로7/7을 확인했다. 원시 이벤트명 유출도7pass/1fail→닫힌 other로8/8을 확인했다. 최종8개에는 handled 부재·중복 event 관측, 역순 응답/handled 완료, 거절/동기 예외 보존과 실제 ReadableStream body 미소비가 포함된다. Node test runner의 초기 spawn EPERM은 환경 실패이며 행동 Red에 합산하지 않는다. 직접 Node 실행으로 실제 Red/Green을 관측했고 동일 명령을 check/CI에 연결했다.

최초 production PC/mobile 각4회·총8회는 기존10초·재시도0·workers2로31.1초에 통과했다. 이후 fresh DB 전체 `offline154-validation-20261017035949`는 Rust430·웹305/91.2% 뒤 브라우저114pass/1fail/2미실행·5.2분이었다. 기존 #83 mobile load 실패를 재현했다. 최초 trace/probe는 `.tmp/offline154-first-full-failure`에 복사 보존했으며 전체 성공으로 해석하지 않는다. 별도 docs202/Mermaid34·오류0을 확인했다.

실패 producer528개는 닫힌 필드 allowlist·음수 counter 부재를 확인했다. 이전 revision0의 최초 boot에서 respondWith40/40·handled41/41·waitUntil107/107은 모두 완료했고 마지막 counter는 모두0이었다. 새 revision1은 skipWaiting 요청 시 message waitUntil1·응답0·handled0이었고, 이후 이전 worker가 stopping→stopped→starting→running으로 재시작했다. bootAt은 이전/새 worker/이전 worker 재시작을 각각 구분한다. 원래10초 뒤 양쪽 boot1·active activated·waiting installed이며 skipWaiting 완료는 없다.

관측된 응답 Promise 대기가 실패를 설명한다는 증거는 없지만 body 전송·native idle·브라우저/계측 원인은 확정하지 못했다. #83은 OPEN으로 유지한다. 후속 기본/네트워크 계측 비활성 비교, 전체 최종 집계·정확 head의 원격 checks/실제 artifact 검토와 병합 상태는 이 작업 PR과 당일 로컬 review에 별도 기록한다.

## 코드 리뷰

같은 this/인자의 native 호출 우선·반환/예외 보존, thenable 재실행 금지, body 미소비/미clone/미잠금, 추가 waitUntil/요청/타이머/debugger 부재를 확인했다. WeakMap은 event 수명을 강하게 잡지 않으며 callback은 최소 info만 보유한다. 제품 SW/coordinator/activity/core/lockfile diff는 없다. 추가 console 관측의 scheduling 영향과 VM이 실제 브라우저 전체를 대체하지 못하는 한계는 유지한다. `/review`·원자료는 gitignore 대상으로 보존한다.
