# #105 · 새로고침 후 본인 최신 결과 발견 검증

[ADR0048](../adr/0048-latest-personal-result.md) · FR01/11/14/16 · NFR01/02/05/06/07/08 · TS15/16/20/21/26/31/35/36. 선행103/PR104 develop9eeb36c 병합·종료 확인 뒤 시작했다.

## 구현과 실제 TDD

Rust latest 입력은 닫힌 `{v:1}`, 응답은 기존 PersonalResult 또는 명시 null이다. 두 API의 같은 Context가 인증/CSRF·닉네임·철회 권위·동시/rate·전체2초를 공유한다. PgResultReader는 본인 human 최소 칼럼만 단일 snapshot/LIMIT2로 읽고 입장 anchor→실제 저장→UUID 내림차순으로 선택한다. 최신 오염/중복은 오류이며 과거 행으로 대체하지 않는다. 웹은 홈 링크→고정 `/{locale}/results`→기존 Atomic 결과 화면이며 UUID는 메모리에만 둔다.

HTTP 신규2개가 기존404에서 실패하고 구현 후 통과했다. 실제PG 신규3개가 Unavailable stub에서 실패하고 단일SQL 구현 후 통과했다. 웹 HTTP 신규2개/누락 controller의 실패와 UI3개의 링크·route 부재를 확인한 뒤 구현했다. UI의 기존 번역 키/명칭과 테스트 대역 타입을 맞춰 UI3개·관련 HTTP/controller/decoder16개 및 전체 타입/lint 검사를 통과했다. 시험 metadata probe의 sqlx JSON 미지원(E0277)은 text→JSON으로 바로잡았으며 의존성 변경은 없다.

## 실제 DB·HTTP·프로세스

로컬 PostgreSQL17.4 전체61개/ignored0/7.58초와 Clippy all-targets를 통과했다. 최신 선택 신규3개는 다른 계정/봇 제외·삭제·legacy NULL anchor·늦은 recovery와 최근 플레이 순서·정확7776000초 open boundary·미래/잘못된 UTC·동일시각 stable UUID·오염된 최신/중복 본인 seat 거절을 검사한다. 기존 실제 erasure barrier 시험은 latest/ID 양쪽에서 저장 후 삭제된 응답을 거절한다. 실제 main OS 재시작 시험에도 ID 없는 latest를 연결해 기존 최소 결과와 동일함을 확인했다.

HTTP18개는 두 API의 initial/post proof·철회·닉네임/세션/계정 교체·전체2초와 혼합 호출의 같은 동시/rate 상한을 확인한다. latest의 query/unknown 필드/중복v·MIME/헤더/Origin/CSRF는 저장소 IO 전에 거절한다. unavailable/권한 실패를 null로 만들지 않으며 disabled503/no-store를 유지한다.

실제 query를 PREPARE한 EXPLAIN ANALYZE에서 기존 online_player_account→online_match_results_pkey nested loop→정렬→LIMIT를 관측했다. 시험 public DB411개 결과/선택 계정1개 행에서 실행0.101ms·shared hit12·sort25kB였다. 고정 반환1개가 DB CPU 상수시간을 뜻하지 않으며 계정별 후보 정렬 비용이 있다. 이 계획 관측은 하드웨어26·운영 부하/RTT 성능 달성 근거가 아니다. 새 인덱스/migration은 추가하지 않았다.

## 실제 새 브라우저 결과 발견

실제HTTPS 대전/WS 뒤 restart control 부재 ECONNREFUSED Red를 확인했다. 시험 전용 loopback supervisor가 example binary를 직접 OS SIGKILL→exit 대기→다른PID→ready 순서로 재시작한다. 같은DB/키와 현재 시험UTC만 유지하고 새 monotonic game clock은0이다. seed/board/UUID/actor를 복사하지 않는다. 기존 고정HTTPS proxy/serial suite/timeout/retry를 유지한다.

known/unknown×390/1440px4개가26.2초/재시도0으로 통과했다. 인증 쿠키만 새 context에 전달하고 새 page `/en/results`에서 v1만 POST하여 본인 결과를 발견했다. 정상 timeout은 기존 통계/completed를 보존하고 unknown은 server_failure/abort/false·own/elapsed null·DB seed 없음이다. 과거 hash/입장 anchor·active0/final1·두 번째 재시작 불변·타계정null·offline 즉시 숨김·8언어·URL/일반storage ID 부재·lobby/WS 자동 재입장 없음·가로 넘침 없음을 검사했다. 원래 SQL unknown fixture나 기존 메모리 ID 조회와 구분한다.

실제 latest-unknown-390/latest-known-1440 화면을 열어 공통 토큰·카드·행동 계층과 통계 null 표시를 확인했다. 원본 디자인은 수정하지 않았다. 실패 trace와 관측 화면·프로세스 JSON은 ignored `.tmp/`에 보존한다.

## 리뷰와 남은 확인

pm-skills code-review의 correctness/changes·baseline develop9eeb36c로 writer→SQL→HTTP 권위→bounded body/proof→controller→UI 및 supervisor→DB를 검토했다. 적용된 historical hash/입장 시각과 저장 순서를 다르게 만들고, 이전 요청보다 새 요청을 먼저 완료한 뒤 슬롯을 재사용하여 상관관계/authority를 반증했다. latest-null에도 post proof가 필요하며 old finally가 새 슬롯을 지우지 않는 것을 확인했다. 현재 근거 있는 미해결 범위 내 정확성 결함은 없다.

전체 회귀 `scripts/check.ps1`은 실제 종료 코드0으로 완료했다. fmt/Clippy·Rust315개·WASM/types/fixtures/tokens freshness·format/lint/typecheck·211unit37파일·production build·94browser3.2분/재시도0·docs155/Mermaid34실패0이 통과했다. 웹 줄 커버리지는2352/2665=88.25%이며 최신 결과 페이지/controller는 각각100%다. recorded tie/키보드 retry·비정상exit 확인·실제 main latest·actual erasure 최신 조회도 최종 검사에 포함됐다. 첫 실행은 번역JSON8개 서식에서 중단돼 이를 수정했으며 실패 로그는 보존했다. 첫 실행의 외부 PowerShell harness가 null ExitCode를 성공처럼 반환했으므로 로그로 실패를 판정했고, 최종 harness는 Handle을 보존하고 null 코드를 거절해 실제0을 확인했다.

최종 실제 프로세스 관측4개는 exit_signal SIGKILL과 PID 변경, 기존 suite가 진행시킨UTC1800004833/5076/5079/5322를 그대로 새 프로세스에 전달한 기록을 남겼다. 초기 상수UTC로 돌아가 시험을 통과시킨 것이 아니다. `.tmp/online-layout/latest-process-*.json`은 시험 전용 관측이며 일반 브라우저 저장소에 기록되지 않는다.

새 독립 CARGO_TARGET_DIR의 실제 DB 포함 서버 coverage도 Exit0이다. 실제PG61개7.66초/ignored0, Auth385/387=99.48%·Match287/289=99.31%·Lobby362/368=98.37%·Result58/58=100%·전체 서버6824/7340=92.97%로 각domain95%/전체80%를 통과했다. 결과 HTTP241/247=97.57%, PG reader99/103=96.12%다. 이전 작업의 coverage object를 재사용하지 않았다.

최신 CI/head·PR 병합/종료는 제출 뒤 확인한다. 상위18은 전체 통합 출구 감사 전 OPEN이며 원인 미확정83·실계정42/하드웨어26/사람27·물리삭제/백업25는 별도다.
