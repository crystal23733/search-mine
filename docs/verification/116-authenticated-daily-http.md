# #116 · 인증 데일리 완료 HTTP

선행 #114/PR115 develop04fe9d1 병합·CLOSED 확인 뒤 [ADR0054](../adr/0054-authenticated-daily-submission-http.md)를 코드 전에 작성했다. FR01/09/10/16·NFR01/02/05/07/08·TS17/18/20/21/31/35/36에 대응한다. 로컬 구현·코드 리뷰·검증은 완료했고 원격 PR/최신CI/병합은 다음 절차다.

## 구현과 관찰 가능한 결과

고정 POST definition/submit/own이 현재 계정·공개 session revision·nickname·Origin/CSRF를 검증한다. accepted PG snapshot을 공유 Rust native 검증기에 전달하고 first-valid/원래 attempt 재전송/후속 Practice·본인 조회를 연결했다. 모든 완료의 trust는 verified_completion이며 client elapsed는 개인 replay 참고값이다. 오류와 정상 own:null을 구분한다.

Rust DTO→생성TS, daily/boundary의 strict 입력/proof/receipt/clock 정책, service의 게시/검증/저장 연결, http의 전체2초/동시4/20회초/철회 장벽을 분리했다. 같은 bounded verifier의 실제2개 CPU permit을 JSON decode·replay·encode가 공유한다. main과 실제 HTTPS fixture에 같은 production router/PG/native·별도 daily authority invalidator를 조립했다. 신규 의존성/추가 개인정보·비밀번호·게임 규칙 복제는 없다.

## 실제 로컬 검증 · 2026-10-11

| 검사 | 실제 결과 |
|---|---|
| scripts/check.ps1 | 실제 종료코드0; fmt/Clippy all-targets -D warnings·WASM·생성타입/fixture/token·format/lint/typecheck·unit/build/E2E·docs/Mermaid 통과 |
| workspace Rust | 358개 통과. 이 실행의 ignored DB85개는 아래 실제 DB 실행으로 별도 검증 |
| PostgreSQL17.4 전체 회귀 | 85개 통과,12.00초,ignored0 |
| 실제PG HTTP/native/rights | 2개 통과,1.62초 |
| daily HTTP | 7개 통과,5.37초 |
| daily 도메인·worker | 22개 통과,2.83초 |
| 웹 unit | 38파일220개 통과,Lines88.83% |
| 전체 실제 브라우저 | 100개 통과,3.7m,재시도0 |
| 개별 실제 HTTPS | PC1440px1.7초/mobile390px1.3초,2개/7.7초,재시도0 |
| 새 target coverage | 실제PG85개/12.15초 포함. Daily353/35699.16%·Privacy160/160100%·서버8309/890393.33% |
| 기존 coverage gate | Auth99.48%·Match99.31%·Lobby98.37%·Result100%; 임계값 domain95/server80 유지 |
| 누락 coverage report | 새boundary를 제거한 보고서는 실제Exit1 missing domain |
| 문서/Mermaid | Markdown166·제품21·설계18·추적36 / Mermaid34개·실패0 |

실제PG HTTP는 첫 완료/동일 attempt 원본 receipt·유효 changed payload conflict409·다른 attempt Practice·본인/타인 own·games/raw export·실제 계정 erasure cascade를 확인했다. 실제 native worker가 살아 있는 동안 logout/erase를 완료한 뒤503/빈 성공 없이401과 write0/삭제 비복원을 검사했다. DB의 기존 unique/잠금/7일90일/동시성 전체 회귀도 통과했다.

실제 HTTPS에서는 fixture의 현재 UTC 날짜에 browser WASM을 생성하고 공개 칸을 순회해 완료한 replay를 production HTTP/PG/native에 제출한다. first/retry/Practice/own·타계정401·verified_completion과 설정 파일의 본인 receipt/raw를 확인한다. 테스트용 제출 호출이며 사용자 웹의 자동 전송/시간 순위 구현은 아니다. 외부 provider proof만 fixture port를 사용한다.

## Red → Green과 correctness 리뷰

route404 대503의 실제 Red0/1 뒤 키 없는3경로503 Green을 확인했다. correctness/changes, 기준 develop04fe9d1에서 요청→accepted definition→worker evidence→PG receipt→전후 session proof/barrier→실제 HTTPS/권리 export를 검토했다. accepted rules가 proposal과 다를 때 accepted 응답과 다른metadata 거절을 강제로 검사했고 기존 worker의 두 요청 reorder/실제 permit 수명 검사를 유지했다.

확정 finding: 대체 ReplayVerifier port가 다른 입력의 sealed 성공 evidence를 반환하면 현재 요청의 canonical payload와 연결되지 않은 채200/저장될 수 있었다. `.tmp/daily116-binding-red.log`0/1·0.33초(실제200/예상503)로 재현했다. 기본 Native 검증은 입력을 재생하지만 adapter의 상관관계 보장이 없었고 downstream receipt 실패는 이미 발생한 write를 막지 못한다. ADR를 보완하고 실제 worker 안에서 accepted metadata와 현재 canonical typed replay를 대조한 뒤 저장을 허용했다. 다른 payload/definition은503/write0이며 HTTP7개 Green이다. 원문 공백/키 순서는 canonical 정규화한다.

수정 뒤 기존 worker fixture의18pass/4fail은 '{}'에 golden evidence를 성공으로 반환하던 입력이었다. 성공 입력을 실제golden으로 바꾸고 기존2초/CPU2/취소/panic/reorder 조건을 유지해22개 Green을 확인했다. 최초PG1pass/1fail은 golden 마지막 입력보다 이른 final_time3300이라400이었다. 유효u32MAX 변경으로 attempt conflict409를 검사했고 제약을 완화하지 않았다. 최초 browser1fail/1미실행의 action.kind를 생성LocalInput의action.type으로 바로잡고 CellState.closed를 사용했다. 모든 실패 로그는 로컬에 보존했다.

coverage는 새 target에서 측정했고 새boundary뿐 아니라 application service도 gate에 포함했다. service109/11297.32%·boundary140/140100%·bounded53/53100%·HTTP153/15996.23%다. 측정 이후 Rust source/test 변경은 없다. broad security audit·실기기 성능·사람 풀이를 검증했다고 주장하지 않는다.

## 다음 절차와 제한

원격PR/최신head checks·exact-head squash merge·116 CLOSED 별도 조회는 아직 완료 전이다. 검토 기록은 gitignored 로컬 review/2026-10-11.md에 이어 쓴다. 공식 시간의 ticket/heartbeat·두 순위·8언어 명시 제출/대기 복구와 상위19 통합 출구는 후속이다. 실제 OAuth42·물리 cleanup/백업25·하드웨어26·사람27·미해결SW83도 별도다. 다중 페이지 export를 단일 snapshot/무제한 파일로 보장하지 않는다.
