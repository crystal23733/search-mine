# #190 · 브라우저 동의 관리와 선택 전송

[ADR0088](../adr/0088-consent-and-event-client.md) · FR01/11/13/16·NFR01/02/05/06/07/08·TS19/20/21/22/24/26/29/31/35/36.

최신develop32e3cad clean/origin0:0·188/PR189 CLOSED/MERGED·열린PR0 확인. 설계/시나리오를 제품 코드보다 먼저 기록했다. 실제Red/Green·검사/코드리뷰·CI/정확head·병합 결과는 실행 후 기록한다. 실제CMP/SDK/광고·외부147/42·상위21/83 완료가 아니다.

## 실제 구현과 검증

self scope의 ConsentControlPort/HTTP는 현재 조회·서버 재검증/CAS·목적별/전체 철회와 bounded/폐쇄 응답을 연결한다. 기존 내정보 동의 decoder를 공유하며, 철회 뒤 grant를 유지하거나 이전 revision을 갱신 성공으로 돌려주는 응답도 거절한다. privacy controller는 작업 시작에 기존 view를 비우고 취소/계정변경/이탈/오류·늦은 완료를 반영하지 않는다. MeConsent의 원래 monotonic expiry와 파일제거를 유지한다.

Collector는 현재 브라우저 purpose와 직전 서버 read의 현재 grant를 모두 요구한다. Rust의 collection_purpose를 서버 domain과 생성 TS map에서 공유하며 목적 규칙을 손으로 복제하지 않는다. 인증/동의·브라우저 수명/라우트 변화와 mutation 시작은 즉시 취소한다. 메모리 탭키와 단건/전체2초만 사용하며 queue/retry/영구저장/SDK는 없다. root는 CMP factory 없이 선택요청0을 유지하고 공개 landing만 연결한다. 동적 module 준비 전부터 mutation을 취소하며 offline import 실패는 명시 재시도할 수 있다.

- 행동 Red: HTTP4개 실패/1개 통과(현재 조회·철회·충돌·receipt). collector의 허용 전송/취소/탭키 실패를 확인했다. 코드 리뷰 중 추가로 빠른 cancel→새화면 기록이 false로 버려지는 실제 Red1을 실행했고, 즉시 슬롯 해제와 이전 finally의 새탭키 비간섭으로 Green을 확인했다. 초기 집중 테스트의 전체 coverage gate 실패는 제품 행동 실패와 구분했고, 이후 집중 검사는 coverage 비활성·전체 gate는 별도로 실행했다.
- 최종 웹397개 통과. 현재 self/닉네임없음·폐쇄 snapshot/receipt·오염/외계정/충돌/철회ack·계정/동의취소·직전 grant/광고·분석 목적분리·만료/시계역행·2초/1건/늦은완료/새탭키·이탈UI·landing/수명 정리 등을 확인한다. 로컬 전체 라인 커버리지는91.17%(3678/4034)이다.
- 전체 Rust workspace464개 통과/실DB185개 ignored, 별도 PostgreSQL18 통합185개 통과(17.04초, 관측1개 filtered). fmt/clippy alltargets·WASM·생성TS freshness 통과. wire shape/DB migration/의존성 추가는 없다.
- 실제 HTTPS3개 통과(8.6초): 기존 동의/선택API 회귀와 신규 privacy 화면의8locale×PC960/mobile360·키보드Enter로 실제 전체철회·폐쇄body/거부결과·가로넘침0·실CMP없는 선택요청0·규칙화면 계속을 확인한다.16스크린샷을 기존 auth-layout 경로에 보존하고 ko360/en960을 직접 시각 검토했다.
- 최초 HTTPS는 고정fixture시각 안에16화면의 read/withdraw를 실행해 실제429에 걸렸다. 원본 trace가429를 확인하며 .tmp/consent190-https-first-failure에 보존했다. 각 시나리오 전 fixture clock을1초 전진시켜 실제 요청 제한을 유지한 채 검증했다. 제품 timeout/retry/속도 제한·브라우저 flags는 변경하지 않았다.
- lint/typecheck/format·production build·문서235개/Mermaid35개 통과. 로컬 PowerShell 전체와 LLVM/전체브라우저는 미실행이며 PR CI에서 커버리지/전체검사·최신head를 확인한다.

## 코드 리뷰와 남는 범위

전송 시점의 current 서버 revision과 같은 저장 transaction이 최종 권위이며 UI/CMP view가 이를 대신하지 않는다. 생성 목적 map과 서버 메서드가 같은 Rust 원천을 사용한다. auth self scope·CSRF·최소 응답·메모리 수명/cleanup·동적 import의 mutation 취소·원래2초·현재 컨텍스트/시계·실패/이탈 상태 비우기를 실제 diff에서 검토했다. 취소 직후 새 요청과 이전 finally 경합은 추가 Red/Green으로 보완했다. 전체 게임 규칙/파일 다운로드·SDK/자동retry/DB/secret은 변경하지 않는다.

현재 CMP factory/서버 verifier를 운영에 연결하지 않았으므로 실제 선택 데이터 수집은 계속 차단된다. landing 외 이벤트 producer·실CMP/TCF/지역/익명subject·운영147/42·상위21/83은 후속이다. 코드 기반 검증을 실제 CMP 성공·법적 정책/출시 출구 완료로 보고하지 않는다. PR CI·정확head·최종 리뷰/병합 상태는 GitHub PR과 로컬 당일 review에 기록한다.

최초 원격 Product41847041379(source2eb6260)는 웹397 통과 뒤 신규 HTTPS의 response.json/CDP Network.getResponseBody unavailable로 최초/재시도 실패했다(100PASS/1FAIL/27미실행,9.5분). 별도 기존 offline 비교8개56.5초 통과를 제품 해결/전체 성공으로 해석하지 않는다. 서버/DB·커버리지는 통과했지만 PR은 병합하지 않았다. 원본trace2/네트워크/로그/layout을 .tmp/consent190-ci-first-*에 보존했다. 실제 200응답 metadata와 UI의 current/두목적 denied 표시를 검증하도록 테스트 관측을 수정했다. 실제 UI가 본문을 소비·검증하고 전후 인증을 마쳐야 성공하므로 CDP 본문 재조회에 의존하지 않는다. 결과는 새 head에서 다시 확인한다.

관측 수정 뒤 로컬 신규 HTTPS1개7.7초·typecheck/format/문서 검사를 통과했다. 실제 서버응답status200·요청v1/all·no-store와 UI current/denied2개가 함께 충족돼야 성공한다. 제품 코드는 변경하지 않았다. 최종 원격 head의6checks는 별도 확인한다.
