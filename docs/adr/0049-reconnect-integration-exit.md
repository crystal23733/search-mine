# ADR0049 · 양쪽 이탈 통합 검증의 종료 관측

- 상태: 채택 · 2026-10-11 · #107 / 상위 #18
- 선행: #105/PR106 develop1f840bf 병합·이슈 종료 확인
- 추적: FR14/16 · NFR01/02/05/06/07/08 · TS15/16/20/21/26/31/35/36

## 배경과 결정

core의 두 명 disconnect 만료는 abandoned/draw/completed:true이며 통계는 알려진 값이다. 실제 HTTPS에서 한 명 forfeit와 OS 재시작 unknown abort는 검증됐지만 양쪽 이탈부터 SQL·새 브라우저 결과 발견까지의 통합 근거가 없다. 기존 규칙/공개 DTO/스키마를 바꾸지 않고 이 출구를 추가한다.

시험 전용 고정 loopback HTTPS proxy에 정확 GET `/__fixture/transport` 관측을 추가한다. 응답은 전역 accepted/server_ended/pending/forced 수뿐이며 UUID·계정·쿠키·board·seed를 담지 않는다. 새 운영 endpoint/기능은 없다. 실제 upstream TCP EOF만 server_ended로 세며 client close·오류·강제 destroy를 정상 서버 종료로 세지 않는다. client 종료 시 upstream write half를 end하고 bounded2초 후 남은 연결만 강제 정리한다. 업그레이드 뒤 기존 request destroy가 관측 전에 socket을 제거하지 않게 한다.

두 브라우저의 실제 WS playing과 proxy pending2를 확인하고 인증 쿠키만 보존한다. 같은 시험 clock을 멈춘 상태에서 두 context를 닫고 server_ended+2/pending0/forced 증가0을 bounded poll한다. 운영 WS 함수가 종료되면 MatchConnection Drop이 기존 ingress 잠금 안에서 현재 시각의 Disconnect를 enqueue하며, upstream EOF는 그 뒤 관측된다. 그러므로 이후 clock 진행의 Tick보다 먼저 같은 시각 Disconnect 둘이 들어간다. client close만이나 고정 sleep을 서버 처리 근거로 삼지 않는다. 테스트 actor 내부 state/정답을 읽는 공개 함수를 추가하지 않는다.

새 context 두 개에는 인증 쿠키만 주고 고정 results에서 본인 최신 abandoned/draw/true·known own/elapsed를 관측한다. 이전 UUID는 시험 expectation/metadata 조회에만 쓰며 앱 URL/입력/storage로 넘기지 않는다. 실제 DB active0/final1/known players/입장 anchor를 확인하고 추가 clock·재조회·실제 OS 재시작 후 동일 결과/시각/통계 불변을 검사한다. unknown abort와 구분한다. 모바일/PC·8언어·secret/자동 WS 재입장 없음도 확인한다.

## 검증과 종료

probe 부재의 실제 Red부터 실행한다. 기존 timeout/retry를 유지하며 신규 시나리오→전체 browser 회귀·관련 docs/Mermaid/format/type 검사를 수행한다. transport harness 변경의 기존 reconnect/recovery/OS restart 영향도 전체 회귀로 확인한다. 제품 source를 바꾸면 해당 domain/DB 검사를 추가한다. 마지막 diff/코드 리뷰·최신 CI/head를 확인해 develop PR을 병합하고 #107 종료를 확인한다.

상위 #18 각 인수 조건을 구현과 실제 실행·하위 PR 병합/이슈 종료 근거로 연결한다. 이 PR 병합·종료 뒤에만 충족된 #18을 종료한다. completed 계약과 광고/승률 소비 구현 #20/#23을 구분한다. 원인 미확정 #83·실OAuth #42/하드웨어 #26/사람 #27·물리삭제/백업 #25는 이 감사로 닫지 않는다.

기존 [규칙04](../design/04-game-rules-spec.md)의 서버 장애/시작 전 취소만 완료 카운트 제외 계약과 `PublicEndReason.completed()`/result model의 모든 reason 분류를 확인했다. abandoned draw의 completed는 true다. 최초 검사 초안의 false 기대를 실제 결과와 기존 계약으로 반증하여 보정했으며 제품 규칙 변경은 없다.

관측 순서는 현재 WS 함수 인자와 Drop 구현에서 도출한 추론이다. [Rust Reference의 함수 인자 소멸 순서](https://doc.rust-lang.org/reference/destructors.html#scopes-of-function-parameters)와 [Node net의 원격 end 의미](https://nodejs.org/api/net.html#event-end)를 확인했다. 이 시험의 정상 서버 함수 종료를 대상으로 하며 임의의 커널/네트워크 장애 전체에서 같은 순서를 보장한다는 주장은 아니다. 실제 SQL 결과가 추가 실행 근거다.

PR108 최초 원격 database job의 coverage 재실행에서 기존103 actual main pair helper가 HTTP429 대200으로 실패했다. helper의20ms status poll은 초당 최대50회여서 기존 session/account20회/초를 넘을 수 있다. 정상 준비 상태 관측 간격을100ms(최대10회/초)로 맞춘다. 기존 전체4초 deadline·제품 rate/capacity·timeout·retry는 유지하며429를 성공으로 받아들이지 않는다. 실제 원격 실패는 보존하고 현재61개PG/관련 gate·수정 head 전체CI를 다시 확인한다.


## 최신 결과 본문의 관측 수명 보정

수정 head12d0188의 Product38077155154는 Rust/core coverage/실DB를 통과했지만 web에서 신규390px 이탈 검사의 Response.json이 실패했다. 기존105의1440px unknown 검사도 같은 오류로 재시도됐다. 두 trace의 실제 latest는200이지만 body resource가 없고 CDP Network.getResponseBody가 실패했다. 신규 응답 시작274926.984ms/소요10.674ms 뒤 body 조회274940.304ms, 기존 응답386626.738ms/소요15.205ms 뒤 조회386650.576ms다. 각각 마지막 goto 뒤 추가 탐색은 관측되지 않았다. 일반 오류 문구로 실제 탐색이나 Chromium 내부 원인을 확정하지 않는다. 최초 결과는76passed/2flaky/1failed/17미실행이며96개 통과로 계산하지 않는다. 별도83 offline-mobile load 실패와 진단8pass도 보존한다.

[ADR0043](0043-result-response-observation.md)의 실제 전달 전 관측을 latest에도 적용한다. 신규107과 기존105의 fresh/reload/empty 조회는 one-use route.fetch로 원래 HTTPS POST를 전달하고 실제200/no-store/JSON을 읽은 뒤 같은 APIResponse를 fulfill한다. 별도 공통 시험 helper가 이를 소유하며 fake body·DB 우회·추가 앱 요청·UUID 입력을 만들지 않는다. maxRedirects0/maxRetries0/기존 관측10000ms, 실패의 reject/abort와 trigger/관측 Promise.all을 유지한다. 앱의 decoder/전후 권한 proof와 화면·SQL·OS 재시작·서로 다른 본인 통계 단언도 그대로 검사한다. 제품 source/기한/재시도/SW 설정은 변경하지 않는다.

이미 실행된 원격 실패를 Red로 보존하고 보정 후 두 이탈·네 OS restart 검사를 재시도0으로 실행한 뒤 전체 browser/정적 검사/최신 head CI를 확인한다. APIResponse의 본문은 [Playwright route.fetch](https://playwright.dev/docs/api/class-route#route-fetch)·[APIResponse.body](https://playwright.dev/docs/api/class-apiresponse#api-response-body)의 공개 계약으로 읽으며, 브라우저 응답의 내부 수명 원인을 해결했다고 주장하지 않는다.

## #109 CI에서 관측기의 페이지 수명 보정

Product38112810532의 web은78pass/1flaky/1fail/16미실행이다. 이탈 검사의 restart 뒤 latest는 실제200/동일body/fulfill까지 성공했지만390px reload와1440px fresh에서 checking 화면에 머물렀다. 실패 trace의 후속 auth/bootstrap은 완료 응답이 없는 상태이며 UI 단언은5초 뒤 실패했다. 첫 원격 run을 반복 성공으로 덮지 않는다.

설치된 Playwright1.63의 Page._onRoute는 times:1 마지막 handler 완료 뒤 interception patterns를 비동기로 갱신한다. 앱의 다음 proof 요청과 같은 페이지의 새 관측기 설치가 이 수명 변경과 겹칠 수 있다. trace만으로 Chromium 내부 경합을 확정하지 않는다. 공통 latest 관측기는 페이지마다 한 번만 설치하고 페이지가 닫힐 때까지 pattern을 유지한다. WeakMap으로 페이지 수명에 묶고 호출마다 한 관측 slot을 직렬로 사용한다. 같은 페이지의 동시 호출은 거절하고 각 handler가 캡처한 slot에만 결과를 전달한다. fresh→reload→후속 proof 사이에 interception을 없앴다 다시 만드는 일을 제거한다. 임의 응답/추가 요청/기한·재시도/제품 코드는 바꾸지 않는다.

실제 HTTPS·PG의 PC/mobile 이탈 두 개와 기존 OS restart 네 개를 재시도0으로 반복하고 관련 전체 gate/CI를 실행한다. 결과 수신은 앱 표시 완료를 대신하지 않으며 전후 권한 proof·개인별 통계·SQL 불변·실제 OS restart와 화면 단언을 유지한다. 별도83은 OPEN이다.

명시 관측을 요청하지 않은 일반 latest 탐색은 route.continue로 원래 요청을 그대로 통과시킨다. 기존 OS restart 검사의 마지막 일반 재탐색도 이 경로이며, 관측 slot이 없다는 이유로 정상 앱 요청을 실패시키지 않는다. 설치 후 실제 호출 계약 검사에서 이 조건을 확인했다.
