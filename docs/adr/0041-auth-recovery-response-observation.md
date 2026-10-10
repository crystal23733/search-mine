# 0041. 철회 복구 응답의 관측 수명

상태: 채택 · M3 #18/#91 · FR14/NFR05/08 · TS15/16/20/26/31/35/36. 선행89/PR90 develop c36a4fe 병합·종료 확인.

## 근거와 경계

Product38023948678의 기존 auth-recovery390px 최초 실행에서 실제 logout 후 브라우저 bootstrap의 Response.json이 Network.getResponseBody:No data found로 실패하고 재시도는 성공했다. 최초 trace를 보존했다. 같은 run의 offline-mobile 두 탭 load 실패는 별도 #83이며 해결로 묶지 않는다.

trace의 마지막 bootstrap은 setOffline(false)199225.35ms 뒤199227.62ms 시작/200·1.952ms, 응답 resource body가 없고 CDP read는199234.973ms 실패했다. 마지막 실패 전 새 navigation은 관측되지 않았다. 도구의 ‘navigated away’ 일반 메시지로 실제 navigation을 단정하지 않는다. production SessionRecovery는 null account를 검사한 뒤 후보를 폐기하고 사용한 signal을 취소한다. 이런 응답 수명과 별도의 늦은 CDP 본문 조회는 동일 보존 보장이 없다. Chromium 내부 원인까지 확정했다고 보고하지 않는다.

## 결정과 검증

실제 철회 이후의 bootstrap만 테스트의 one-use route에서 production HTTPS endpoint로 전달한다. route.fetch의 실제 APIResponse 본문을 메모리에서 읽어 status200/account:null을 확인한 뒤 같은 response를 route.fulfill로 브라우저에 전달한다. 앱이 받아 검증하기 전에 서버 응답 내용이 관측되며 브라우저의 후속 signal 취소에 의존하는 CDP 본문 조회를 하지 않는다. 임의 응답 JSON을 만들거나 실제 철회/proof assertion을 제거하지 않는다. 성공 복구·계정/세션 revision/cookie 동일·offline 쓰기0·철회 후 권한 제거/download0·후속 독립 bootstrap:null 검증을 유지한다. 한 번 전달한 route는 제거한다.

수정 전 CI Red와 로컬 반복 결과를 구분하고 PC/mobile 재시도0 반복·관련 전체 browser·CI 로그를 확인한다. 제품/CSRF/권한·30초/10초·timeout/retry/SW/일반 storage는 바꾸지 않는다. 새 response observer의 backend failure/body parsing 오류는 테스트 실패로 드러나야 하며 cleanup에서 pending rejection을 숨기지 않는다. 실제 서버 재시작 복구/미확인 통계는 후속18, 외부42/26/27과 원인 미확정83은 별도다.

Playwright의 [route.fetch](https://playwright.dev/docs/api/class-route#route-fetch)와 [APIResponse.body](https://playwright.dev/docs/api/class-apiresponse#api-response-body) 계약을 따른다.
