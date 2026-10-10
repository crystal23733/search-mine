# #95 본인 결과 응답 관측 검증

M3 · FR11/14/16, NFR05/08 · TS15/16/20/21/26/31/35/36. [ADR0043](../adr/0043-result-response-observation.md), 기준 develope5493be·선행93/PR94 병합·종료.

CI38050829104 최초 known1440 Response.json/CDP NoData Red와 trace를 보존했다. 마지막200 body resource 없음·새 navigation 관측 없음·controller 성공 후 signal 폐기의 경계를 확인했다. Chromium 내부 원인을 확정하지 않는다. 로컬 변경 전/후12회 known/unknown·PC/mobile·재시도0 및 전체browser/정적검사/CI는 실제 실행 후 기록한다. 제품/Rust/기한/SW/일반 storage를 바꾸지 않는다.

최초 repeat-each3은 기본3worker로 각 반복이 공용 fixture clock을 동시에 advance하여 playing 기대에 finished가 도착했다(4통과/2실패/6미실행·1.1분). 이 결과를 CDP 관측 재현으로 세지 않고 .tmp/result95-baseline-test-results에 보존했다. 원래 serial 전제를 유지하는 workers1로 변경 전12회1.0분·변경 후12회1.0분, 모두재시도0 통과했다. 제품 병렬성/Playwright config/timeout/retry는 변경하지 않았다.

소스 검토에서는 AuthenticatedRequests.execute의 finally가 외부 cancel listener를 결과 반환 전에 제거한다는 반례도 확인했다. controller의 외부 request 폐기가 실제 fetch abort를 일으킨다는 원인 가설은 채택하지 않는다. 마지막200 body resource 부재/늦은 CDP 조회 실패와 구분해 기록하며, 실제 APIResponse 관측 후 동일 응답 전달 자체를 검증한다. format/lint/typecheck/docs145/Mermaid34실패0 통과. 최신 build/전체browser·최종 리뷰/CI는 진행 중이다.

최신 pnpm build 뒤 실제 PG17.4/HTTPS 전체90browser2.6분·재시도0 통과했다. one-use route.fetch의 실제200/no-store/최소 known·unknown 본문을 확인하고 동일 응답을 전달한 뒤 기존 UI/권한/키보드/503/404/타인404/8언어/storage0·다시하기 단언도 통과했다. 이번 변경은 테스트와 문서만이며 Rust/전체 웹 unit/서버 coverage를 로컬에서 다시 실행했다고 보고하지 않는다.

최종 correctness 리뷰는 develope5493be 대비 테스트와 실제 result HTTP→전후 인증 proof→controller/요청 수명→UI, route.fetch/APIResponse→동일 fulfill와 오류 promise 흐름을 따라 확인했다. one-use·고정 endpoint·redirect/retry0/기존10초·비동기 오류 전달과 원래 행동 단언을 유지하며 검토 범위에 근거가 있는 미해결 finding은 없다. 실제 fetch 취소 원인 가설은 위 반례로 채택하지 않았고 브라우저 내부 원인을 해결했다고 주장하지 않는다. CI/PR·병합은 원격 완료 후 기록한다.
