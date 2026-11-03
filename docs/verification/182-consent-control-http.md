# #182 · 현재 인증 세션 동의 HTTP 연결

[ADR0084](../adr/0084-bounded-consent-control-http.md). FR01/13/16 · NFR01/02/05/07/08 · TS20/21/24/29/31/35/36.

선행20/174/178의 병합·CLOSED와 develop90a55b9 clean·원격0/0·열린PR0을 확인하고 설계/시나리오를 제품 코드 전에 기록했다. `feature/182-consent-control-http`에서 구현했다.

## 변경과 코드 리뷰

현재 opaque cookie의 세션으로 동의 조회·서버 verifier 재검증·목적별/전체 철회 POST를 연결했다. account/subject/grant/TC 원문 입력을 받지 않는 폐쇄 v1 DTO와 canonical non-nil expected revision을 Rust 원천에서 생성 TS로 내보낸다. expected revision은 CAS 조건이고 수집 권한이 아니다. 닉네임·최근300초 재인증이 필요 없는 일반 권리를 기존 Origin/CSRF와 현재 세션으로 보호한다.

본문 수집을 포함한 전체2초·4동시·4KiB·session/account20회/초·rate key4096 한도와 기존 Me 권위 registry/인증 invalidator를 공유한다. 요청 전후 같은 세션·시각/기한·철회 barrier를 확인하고 fixed-size/no-store/no-referrer/nosniff 응답을 만든다. 만료/미확인/장애를 구분하며 기존 내 정보와 같은 Rust consent projection을 사용한다. 기존 Me의 세션 유효성 검사를 Auth Session에 옮겨 두 HTTP가 공유하고 기존 행동을 보존했다. 옮긴 projection도 consent domain95% coverage 검사에 포함했다.

목적별 철회는 다른 목적의 원래 만료를 연장하지 않는다. 전체 철회는 한 transaction/새 revision으로 두 목적을 차단한다. 기존 account→session 잠금 순서·재검증 전 차단 commit·검증 IO 잠금 해제·늦은 CAS 거절을 유지한다. timeout/caller 취소/검증 장애 뒤 차단을 복원하지 않고 logout/계정 권리의 cascade와 취소 guard를 재사용한다. main·실제 HTTPS fixture는 PgConsentStore와 DisabledConsentVerifier를 사용한다. 테스트의 허용 verifier는 실PG 테스트 주입에만 있다.

새 의존성·migration·개인정보·이력 저장은 없다. 개인 파일 다운로드를 되살리지 않는다. 실제 CMP·선택 이벤트 저장 시점 권한·익명 subject·운영 설정147·상위21 전체·83 해결은 별도 미완료다. 현재 응답이나 브라우저 선택을 선택 수집 권위로 사용하지 않는다.

## Red → Green

실제 stub 실행에서 폐쇄 입력2개·HTTP3개·전체 철회1개·비설정 기본 합성1개의 행동 실패를 확인한 뒤 구현했다. HTTP 경계8개·Consent 모델7개·기존 Me 모델7개·wire2개가 통과했다. 느린 body가 실제 동시 예산을 차지하고 원래2초 만료/취소 뒤 예산이 복귀하는 동작도 검사한다. 컴파일/테스트 준비 오류를 행동 Red로 계산하지 않는다.

실PG4개는 HTTP 허용 verifier→Disabled 재검증503/사전 차단, 원자 전체 철회와 늦은 최초 grant409, 다른 목적 보존/기한·caller 취소, 실제 logout의 대기 HTTP401/cascade를 검사한다. 최초 PG 검사 중 시각1900에 고정1500 CSRF를 사용한 fixture 오류403은 현재 시각 CSRF로 고쳤고 제품 권한 검사를 완화하지 않았다.

## 실제 로컬 검증

| 검사 | 결과 |
|---|---|
| Rust workspace | 451 passed, 실DB160 ignored (별도 실행) |
| PostgreSQL18 전체 | 160 passed / 17.00초; 새 동의 HTTP4개 포함 |
| 웹 전체 단위 | 52파일·372 passed; line3444/3770=91.35% |
| 실제 HTTPS 새 동의 경계 | 1 passed / 5.9초; 닉네임 전·301초·현재 view·철회·미설정503·logout401 |
| 생성/정적 | fmt·clippy workspace/all-targets·WASM check·생성 TS/fixture/token freshness·Yarn format/lint/typecheck/build 통과 |
| 워커 진단 | worker-probe8·native-worker-trace8 passed |
| 문서 | 227 Markdown·제품21·설계18·시나리오36, Mermaid35/오류0 |

최초 비승격 Rust 실행은 기존 로컬 WS bind에서 OS PermissionDenied로 중단됐고 필요한 loopback 권한으로 전체를 다시 실행해 통과했다. 실제 HTTPS auth 전체 최초 실행은22 passed/1 failed/39 not run으로 전체 통과가 아니다. 기존 unknown-abort390 검사에서 cleanup probe GET이 predicate5초 내 완료되지 않았고 원본 trace/error-context를 `.tmp/consent182-first-auth-failure/`에 보존했다. 원인 분리와 재실행 결과는 아래 및 PR에 기록한다. timeout/retry/브라우저 플래그를 완화하지 않는다.

로컬 PowerShell check.ps1 자체·LLVM coverage·전체 browser126개는 미실행이며 정확 head의 CI와 구분한다. 외부 OAuth/CMP·실제 사람/운영 하드웨어 증거로 해석하지 않는다. 로컬 상세 로그는 무시되는 `.tmp/consent182-*`와 당일 review에 보존한다. 최종 CI·리뷰·PR/병합·이슈 종료는 실제 결과를 PR에 기록한다.

동일 cleanup 검사는 기존 fixture의 공유 시계를 겹치지 않게 한 직렬5회에서 모두 통과했다(21.8초). 잘못 병렬화한 repeat-each 진단에서는 공유 시계가 겹쳐 playing 대신 finished3개/통과2개가 발생했으므로 최초 cleanup 실패의 재현으로 계산하지 않는다. 제품 설정은 그대로이고 최초 원인은 미확인이다. 새 DB의 전체 auth와 정확 head의 CI를 계속 확인한다.
