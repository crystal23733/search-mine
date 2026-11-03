# #188 · 현재 세션의 선택 이벤트 수집 HTTP

[ADR0087](../adr/0087-optional-event-http.md) · FR01/13/16·NFR01/02/05/07/08·TS20/21/22/24/29/31/35/36.

선행184/186·PR185/187 병합/CLOSED, 최신develop8b1d1d6·clean·원격0:0·열린PR0 확인. 설계와 시나리오를 제품 코드보다 먼저 기록했다. 실제 Red/Green·코드리뷰/검사·CI·PR/병합 결과는 수행 후 기록한다. 실제CMP/브라우저수집·광고·외부147·상위21/83 완료가 아니다.

## 구현과 실제 검증

POST /api/v1/events/optional은 현재 cookie/Origin/CSRF와 서버가 선택한 계정·세션으로 단건을 저장한다. 기존 이벤트 wire를 generic ID Rust protocol로 공유하고 생성 TS를 갱신했다. 목적 도출/시각/locale/권한 정책은 서버 도메인에 유지했다. 공개 receipt는 v/event_id/원래received_at뿐이다. 본문2048·정규화draft1024·동시4·세션/계정별초당20·추적4096·no-store를 적용한다. 원래2초를 전후 세션·pool·DB까지 전달하고 기존 직접 capture도 기본2초를 유지한다. main과 HTTPS는 실제 verifier Disabled를 유지하며 브라우저 자동전송·광고/CMP·익명subject를 활성화하지 않는다.

- 행동 Red: 기존 미설정 경로404대503, 연결stub503대정상200/권한403의3개 실패를 실제 실행했다. 컴파일 오류는 Red로 계산하지 않았다. Green3 뒤 경계 확대 HTTP10개 통과(0.01초).
- 실제PG 신규5개 통과(2.55초): 동시중복 한행/불변receipt·다른내용409·미확인/광고목적/철회403·타계정/낡은revision·로그아웃cascade·미설정/과도deadline거절·기본호출복귀. 본문750ms 뒤 세션SQL/저장 작업 및 guard취소에서도 blocker를 풀기 전에 원래 backend종료와 저장0을 확인했다. 더 보강한 타계정/낡은revision 시나리오는 전체PG에서 다시 실행했다.
- 전체 Rust workspace464개 통과·DB185개 ignored. 별도 PostgreSQL18 통합185개 통과(16.86초, 관측1개 filtered). ignored를 실행한 것처럼 합산하지 않았다.
- 웹52파일372개 통과, 라인91.35%. fmt/clippy 전체target·WASM·생성TS 일치·format/lint/typecheck·production build 통과. 문서233개·Mermaid35개 통과.
- 실제 HTTPS 신규1개 통과(3.3초): nickname 없는 현재세션의 미동의/명시거부403·위조400·CSRF403·로그아웃뒤401·실CMP없는refresh503·페이지 이동 중 자동수집0·no-store를 확인했다. 최초 테스트는 unknown revision을 nonnull로 잘못 가정해400대403으로 실패했고, 명시 전체철회로 거부 revision을 만든 뒤 확인하도록 테스트 준비를 수정했다. 원본trace/errorcontext/log는 로컬.tmp/events188-https-first-failure에 성공과 별도 보존했다. 이는83과 별개다.
- 로컬 PowerShell 전체 스크립트·LLVM 커버리지/전체브라우저는 미실행이다. 필수6checks와 커버리지/전체브라우저·최신head는 PR CI에서 확인한다.

## 코드 리뷰

입력은 계정/권한/서버fact/임의필드를 받을 수 없고, 기존 decoder가 canonical ID·8locale·시각/일자/소요시간을 검증한다. expected revision을 읽기 snapshot으로 신뢰하지 않으며 같은 저장 transaction의 현재동의/목적/세션을 다시 검사한다. 원래기한이 본문·세션·pool·DB에서 재시작되지 않고 DB timeout은 caller취소 뒤에도 적용된다. receipt는 요청ID·원래접수시각만 공개하며 전후 같은권한·시계/만료·취소guard를 거친다. 기존 계정권리의 registry를 main/fixture에서 공유한다. 파일다운로드·개인payload/로그·새migration/의존성·브라우저 flags/retry 변경은 없다.

PR CI·정확head·최종 리뷰/병합 상태는 GitHub PR과 로컬 당일 review에 기록한다. 실제CMP/브라우저 collector·광고·전체정책/외부147·상위21·83 완료가 아니다.
