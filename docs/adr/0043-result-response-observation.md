# 0043. 본인 저장 결과 응답의 관측 수명

상태: 채택 · M3 #18/#95 · FR11/14/16, NFR05/08 · TS15/16/20/21/26/31/35/36. 선행93/PR94 develope5493be 병합·종료 확인.

## 관측과 의무

Product38050829104의 기존 known 결과1440px 첫 실행은 브라우저 Response.json/Network.getResponseBody:No data found로 실패하고 retry 성공했다(89passed+1flaky6.5분). 로컬90retry0와 구분한다. 최초 artifact는 .tmp/result93-ci-failures에 보존했다. trace의503/404 body resource는 존재하고 마지막 실제200(fetch215008.331ms/4.958ms)은 body resource가 없으며 CDP read는215019.628ms 실패했다. 마지막 goto213604.026ms 이후 response 전후 새 navigation을 관측하지 못했다. 일반 도구 오류 문구로 실제 navigation/Chromium 내부 원인을 확정하지 않는다.

OnlineController.lookupResult는 실제 결과/전후 인증 proof를 받은 뒤 stop→cancelResultRead로 요청을 폐기한다. 실제 소비와 나중의 CDP 본문 조회는 같은 보존 수명 보장이 아니다. [ADR0041](0041-auth-recovery-response-observation.md)의 실제 HTTP 전달 전 관측 방법을 이 결과 경계에 적용한다.

소스 검토에서 AuthenticatedRequests.execute의 finally가 결과 반환 전에 외부 signal의 cancel listener를 제거하는 것도 확인했다. 따라서 controller의 외부 요청 폐기만으로 실제 fetch signal 취소가 발생했다고 주장할 수 없다. body resource 부재와 늦은 CDP 관측 실패는 실제 관측이며, 요청 취소/브라우저 내부 원인의 인과관계는 미확정이다. 이 구분을 유지하고 테스트의 본문 관측 의존을 직접 제거한다.

## 결정

기존503/404 실험 route를 제거한 뒤 최종 실제 /api/v1/results 요청만 one-use route.fetch로 전달한다. 실제 APIResponse의 status200/no-store와 known 또는 unknown 최소 JSON을 읽은 뒤 같은 APIResponse를 fulfill하고 관측 promise를 완료한다. 앱은 같은 서버 body를 받아 기존 bounded decoder와 전후 세션 proof를 거친다. 임의 JSON·다른 세션·추가 storage를 만들지 않는다. maxRedirects0/maxRetries0/기존10000ms를 사용하며 fetch/body/fulfill 실패는 promise reject와 route.abort로 테스트에 전달한다. keyboard Enter와 관측을 Promise.all로 함께 기다려 오류를 숨기지 않는다.

기존 actor 정리/503/404/명시 키보드 조회·본인 최소 응답/hash·uniform404·철회/삭제·8언어/overflow/secret0/storage0·known/unknown·다시하기 단언을 유지한다. 제품/Rust/규칙/기한/SW/timeout/retry는 바꾸지 않는다. 변경 전 실제CI Red·로컬baseline 반복, 변경 후 PC/mobile known/unknown 반복과 전체browser·관련정적검사·최신CI를 구분해 기록한다.

실제 영속 저장 소유 연결/journal/admission/startup abort/processkill·새로고침 발견은 다음18이며 #83/42/26/27은 별도다. [Playwright route.fetch](https://playwright.dev/docs/api/class-route#route-fetch)·[APIResponse.body](https://playwright.dev/docs/api/class-apiresponse#api-response-body)의 공개 계약을 따른다.
