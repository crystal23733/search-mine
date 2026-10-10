# #82 인증된 본인 완료 결과 조회 검증

FR14/16, NFR01/02/05/07 · TS15/16/20/21/31/35/36. [ADR0039](../adr/0039-authenticated-personal-result-read.md)의 최소 응답과 권위 경계를 검증한다.

선행 #80/81·#85/86·#84/87 병합·종료 확인, 기준 develop `2db4ac78266b22eb8159cdcabfc59af905999c91`.

## Red와 구현

키 없는 경로는 기존 빈 응답으로 JSON 파싱 실패, 정상 canonical 요청은 Malformed, 최소 결과 투영은 Unavailable로 실패했다. UUID의 serde feature가 없는 기존 환경에서 테스트 JSON에 Uuid를 직접 넣은 컴파일 실패는 행동 Red로 세지 않고 문자열로 보정했다. HTTP 정상 조회503→기대200의 실제 Red와 PG17.4 저장 결과 None→기대 본인 결과의 Red를 따로 실행했다.

Rust DTO→생성 TS, completed 단일 함수, ResultReader/검증 투영/Pg·account+match 조건/LIMIT2/90일, 별도 authority·합성 invalidator, 전후 session proof·최종 순수 응답 commit, 전체2초·동시16·bounded session/account rate, 키 없는503/main/config를 구현했다. 기존 migration·저장 retry·제품 게임 규칙·의존성/lockfile은 바꾸지 않았다.

## 실제 관련 실행

HTTP13개 통과: 정확한 최소 응답/타 계정·없는 결과, Origin/CSRF/Content-Type/query/body/version/UUID, 세션/reader 실패·만료·오염, 초기 read 중 철회, DB 중 세션ID/account/생성/만료/재인증/nickname/철회 변경, 요청 상한·취소 해제·3단계 단일2초·token 회전 rate·역순 두 세션·공유 조회 취소·별도 WS 권위·authority 포화·clock 역행. 순수 results 도메인/설정/rate4개와 프로토콜 전체도 통과했다.

실제 PG17.4 신규3개 통과: 양 좌석/봇/저장된 과거 hash·seed null 이후 중복 저장·90일 직전/정각·삭제 참여 행/계정 FK cascade와 retry 비복원·duplicate seat/불가능 reason-outcome·실제 SessionReader/HTTP 양 좌석/봇/동일404·captured result 중 실제 erase_authorized·중첩 WS/로비/result invalidation과 본인401/다른 참여자 조회 유지. 전체 gate/coverage/CI/PR 결과는 실행 후 기록한다.

## 코드 리뷰와 최종 로컬 검사

pm-ai-shipping:code-review의 correctness를 기준 develop2db4ac7 대비 순차 적용했다. 요청과 반환 account/match를 어긋나게 하고, 같은 계정의 두 세션을 겹쳐 후발 bind→이전401/후발200을 확인했다. 같은 세션의 두 조회 중 하나를 취소해도 다른 조회는200이며 별도 WS lease는 유지된다. 기존 writer·participant 삭제/FK cascade·중복 save와 reader, auth 철회·최종 순수 응답 commit, 단위/enum/TS 손실을 연결해 검토했다.

발견한 P2 correctness는 `interval '90 days'`와 경과 초 단위 보존의 불일치다. America/New_York의 DST를 지나는 실제 PG 단일 connection에서90일 정각30분 전 조회가 조기 만료되는 Red를 재현했다. DB timezone이 UTC라는 보장이 없어 반론으로 배제할 수 없었다. ADR에90×86400초를 명시하고 SQL을 `7776000 seconds`로 고쳐 같은 경계가 통과했다. 현재 검사한 범위에서 지원되는 미해결 결함은 없다.

최종 `scripts/check.ps1` exit0: Rust fmt/Clippy/native/WASM·생성 타입/fixture/token·웹 format/lint/typecheck·181unit21.88초(line87.52%)·fresh build·실제PG17.4/HTTPS 포함85browser5.7분/재시도0·docs137/Mermaid34실패0. native 실행 뒤 추가된 DST 수정은 최신 fmt/all-target Clippy와 별도 실제PG 전체30개/ignored0/9.02초로 재확인했다. 앞선 전체DB는 수정 미적용 상태의DST1fail/29pass와, 수정 후 기존 readiness 정리의PoolTimedOut1fail/29pass를 기록했다. 후자의 원인은 미확정이며 timeout/retry/worker를 바꾸지 않고 같은 명령으로30개 성공을 확인했다. 전체 서버 커버리지/CI18/PR 상태는 확인 후 추가한다.

웹 소비·active journal·재시작 abort·물리 보존 정리·OAuth 실계정·미니PC 성능은 이 이슈에서 완료했다고 주장하지 않는다. #83 두 탭 업데이트 제품 원인도 미확정이다.

새 결과 단위 테스트5개의 본문은 `tests/unit/result_*.rs`로 옮겨 source coverage의 분모에서 제외했다. 이동 전후 본문 동일을 비교했고 실행/격리/단언을 유지했다. 최신 fmt/all-target Clippy·전체 서버 `cargo llvm-cov --tests -- --include-ignored`/실제PG30개8.21초/HTTP13개0.91초와 gate exit0. 최종 제품 코드 line 기준 Auth385/387=99.48%, Match287/289=99.31%, Lobby362/368=98.37%, Result53/53=100%, 전체6137/6615=92.77%다. 처음 inline 테스트가 섞인 Result116/116·전체6266/6744 수치는 최종 분모로 사용하지 않는다. 계측 산출물 `.profraw/.profdata`도 gitignore에 추가했다.

새 조회 경로는 실제DB+Axum HTTP 통합으로 검증했다.85개 HTTPS 브라우저는 기존 사용자 흐름의 회귀 검사이며 새 조회의 웹 소비/HTTPS 시나리오는 후속 작업이다. CI PostgreSQL18/PR/병합 결과는 PR 본문과 일별 로컬 리뷰에 확인 후 기록한다.

최종 PR88은2026-10-10T02:29:45Z/develop cdd700e로 병합했고82 CLOSED를 확인했다. CI PG18.6 실제 DB30개/181unit·85browser 재시도0, Result53/53=100%/전체6137/6615=92.77%·최신 checks 통과. 웹 소비는 후속89이며 영속 재시작 출구18은 별도다.
