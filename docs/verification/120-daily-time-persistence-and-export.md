# #120 본인 내보내기와 검증된 데일리 시간 영속화

[이슈120](https://github.com/crystal23733/search-mine/issues/120) · M3 · [ADR0056](../adr/0056-daily-time-persistence-and-export.md).

선행 #118/PR119 develop307a58da21c3c92f9461afca78b41ad69a53895b 병합·#118 CLOSED 2026-10-11T15:01:58Z 확인 뒤 시작했다. 설계/TS/추적을 코드 전에 갱신했다. 구현·로컬 실행 검증을 아래에 기록하며 원격 PR/CI/병합은 다음 절차다.


## 구현과 실제 코드 리뷰

Rust nullable time DTO/생성TS·strict 웹 export를 저장 전에 준비했다. 누락/undefined/문자/음수/소수/u32overflow/최소native elapsed를 거절하고 NULL·Some을 보존한다. 별도 submit_timed의 기본 port는 Unavailable, 실제 PG는 plain/timed 공통 bounded transaction 및 sealed account/attempt/accepted metadata/canonical digest 결합을 사용한다. forward202610120001 nullable bigint/u32 CHECK·기존202610110001 checksum·기존 immutable trigger를 유지한다. time은 accepted duration보다 작고 native semantic 완료 이상이다. 기존NULL은승격하지않고원래재시도/Practice는첫receipt/time을반환한다. owner SQL/DTO/export와 HTTP trust는같은검증된저장값을사용한다.

pm-ai-shipping:code-review correctness/changes baseline307a58d 순차검토. 원래9000과새4000proof·NULL→Some을강제분리하여요청값이원래receipt를덮지않음을확인. 실제12개동시plain/timed·잘못된account/attempt/다른canonicalfinaltick·현재session hash/계정삭제·타인export·forward기존row·UPDATE불변·DB CHECK/손상failclosed·7일raw/90일record를검사했다. 범위내추가뒷받침finding없음이며별도performance/security/실인간/글로벌RTT감사완료를뜻하지않는다.

## Red·Green과 준비 실패 구분

- nullable export 실제Red0/1/1.51초→focused Green10/2.53초. 첫selected Green10개는전체coverage11.78%로프로세스Exit1이었고후속focused coverage-disabled 검사와구분한다. 전체80%기준은최종full에서유지했다.
- 실PG sealed 첫저장 Red0/1/0.77초 Unavailable→첫Green2/1.09초. receipt timebounds Red0/1/0.00초 OkvsUnavailable→전체도메인72/3.25초Green.
- 최초PG시험private canonical_replay 접근과추가HTTP시험PrivacyAuthentication.workers누락은compile101 준비오류였다. 공개golden/기존ExportWorkers로수정했고제품행동Red로계산하지않았다. Clippy의시험counter-loop도zip iterator로수정하고strictClippy통과했다.
- 중간PG회귀91/12.66초 이후추가actualstoredtime HTTP1/1.20초, 최종freshcoverage전체92/13.37초를구분한다. 제품HTTP8/5.38초도통과했다.

## 최종 로컬 검증

최종 scripts/check.ps1 실제Exit0: Rust383·web221/38files/Lines88.85%·browser100/3.7m/재시도0·fmt/Clippy/WASM/types/fixtures/tokens/format/lint/typecheck/build·문서170/추적36/Mermaid34실패0. 새target 실제PG92/13.37초 포함 instrumentation: Daily695/70898.16%·Privacy164/164100%·서버8685/928993.50%, domain95/전체80/entry point 기준 유지. 측정 뒤 Rust source/test 변경 없음.

actual HTTPS의 WASM 완료→completion-only API→본인 nullable time·raw 다운로드를 PC1440/mobile390 각각8언어 설정에서검사했다. Some의실제sealed 저장/본인시간HTTP는PG통합으로검증했고공개timed수집이활성화되었다고주장하지않는다. fixture clock4초진행은기존rate window/현재proof를존중하는시험시간진행이며제품rate/timeout/retries변경없다.

## 최초 전체 검사 실패와 환경 수정

첫 scripts/check.ps1은 Exit1, browser80pass/1fail/19미실행/3.6분이었다. 기존 personal-export PC1440의첫설정다운로드가실패하여download대기30초timeout, 이후context.close 오류가이어졌다. 실제trace는 account export와online game page200뒤daily game page503,settings auth_unavailable을보였다. 브라우저fixture는migration을사전에수행해야하는데새202610120001을개발용public DB에적용하지않은실행준비누락이었다. psql 실제조회는최신migration202610110001/새열0개,README의migrate 실행뒤202610120001/새열1개를확인했다. 원래로그/trace는 .tmp/daily120-first-full-failure 및최초full로그로보존했다. 제품소스/timeout/retry수정없이동일다운로드와별도full을재검증하며이를118resource오류나미확정83SW와같은원인으로분류하지않는다.

## 제한과 다음 절차

공개start/heartbeat/timed-submit·두순위·8언어명시데일리UI/대기는후속이다. seed사전계산/인간풀이/전세계latency공정성을sealedelapsed가증명하지않고주입clock성공을실벽시계성적으로계산하지않는다. 상위19·외부42·물리25·하드웨어26·사람27·원인미확정SW83은OPEN이다. 최초118로컬resource오류를이작업으로해결했다고주장하지않는다. /review/ 일별파일은Git에서제외한다. 원격CI·exact-head병합/120CLOSED는다음절차다.


## 원격 CI·병합 종료

[PR121](https://github.com/crystal23733/search-mine/pull/121) sourceacf1fa9의 Product38152189136/Repository38152189162와 본문 갱신 최신 검사 모두 SUCCESS. 실제PG18.6 92/30.40초·instrumented92/29.88초·Daily695/70898.16%/Privacy100%/서버8685/928993.50%, web221/38files/Lines88.85%, browser100/6.4분(실패·재시도 관측0). 최신8rollup/6필수·3run·정확head/clean 확인 후 2026-10-11T15:51:27Z squash 병합 develop08eede7, #120 CLOSED15:51:28Z 별도 확인. 최초 로컬 browser DB forward migration 준비 누락은 실제 psql 확인·migrate 후 같은 source로 수정/최종 full100을 확인했다. 기존83 미확정 원인과 구분한다. 다음 #122는 공개 시간 HTTP이며 두 순위/UI/상위19는 미완료다.
