# #186 · 동의 HTTP와 실제 DB 작업의 동일 기한

[ADR0086](../adr/0086-consent-operation-deadline.md). FR01/13/16·NFR01/02/05/07/08·TS20/21/24/29/31/35/36.

선행182/184·PR185 병합/CLOSED, 최신 developfdb7247 clean·원격0:0·열린PR0을 실제 확인했다. 설계와 시나리오를 제품 코드 전에 기록했다. 실제 Red/Green·코드 리뷰·검증/CI·PR/병합·종료 결과는 수행 후 기록한다. 실제 CMP/수집기·외부147·상위21/83 완료로 보고하지 않는다.

## 실제 구현과 검증

HTTP 본문 수집 전 생성한 원래2초 absolute deadline을 동의 port·전후 세션 확인에 전달한다. PgConsentStore는 immutable 호출 문맥으로 같은 pool/clock을 공유하며, refresh 사전 차단과 검증 뒤 저장도 같은 기한을 사용한다. 기존 직접 호출의 기본2초는 유지한다. 선택 저장의 transaction-local PostgreSQL18 기한을 공통 infrastructure helper로 추출했다. PgSessionReader는 실제 SQL에도 기한을 적용하며 일반 세션 SQL/hydration을 공유한다. 공개 DTO·생성 TS·migration·의존성·브라우저 설정 변경은 없다.

- 행동 Red: 실제 PostgreSQL에서3개 실패(4.49초). 동의4종 작업, 본문750ms 뒤 저장, 첫 세션 SQL에서 요청 종료 후 backend가 남았다. blocker를 해제하기 전에 관측했다. 컴파일 오류는 Red로 세지 않았다.
- Green: 위3개 통과(2.49초). 추가 경계를 포함한 PG6개 통과(2.60초): 검증 IO·두 transaction의 같은 기한과 사전 차단,4종 caller취소, 만료/과다 기한 거절·기본 호출 복귀·기존300ms 한도·연결 교체 뒤 setting0. HTTP port9개 통과: 본문750ms 뒤 전후 세션/저장에 같은 원래 Instant와 남은1250ms를 확인했다.
- 전체 Rust workspace454개 통과·실DB180개 ignored; 실PostgreSQL18 통합180개 통과(17.91초, 별도 관측 테스트1개 filtered). ignored를 실행한 것처럼 합산하지 않았다.
- 실제HTTPS/쿠키/CSRF/DB 동의 API E2E1개 통과(5.7초). 서버 기본 verifier가 Disabled인 상태의 차단 응답을 확인한다.
- cargo fmt·workspace/all-targets clippy·WASM target·생성 TS 일치 검사·Yarn production build 통과. 문서231개·Mermaid35개 통과.
- 로컬 PowerShell 전체 스크립트와 LLVM 커버리지는 실행하지 않았다. 전체 web/브라우저·커버리지와 최종 head 검사는 PR CI에서 확인한다.

## 코드 리뷰

실제 diff에서 원래 기한이 pool 대기·본문·전후 세션·verifier IO·refresh 두 transaction을 지나도 재시작되지 않음을 확인했다. transaction-local 설정으로 전역/다른 요청에 기한을 남기지 않으며 timeout/caller취소 뒤 실제 backend 종료를 blocker 해제 전에 검증했다. 기존 짧은 DB 기한도 실제 보존된다. account→session 검증·CAS·사전 차단·authority guard·no-store·기존 요청 한도는 유지한다. verifier IO 동안 DB 잠금/연결을 보유하지 않는다. 기존 선택 저장 helper 추출은 같은 SQL/오류/시간 계산이다. 실제 CMP/공개 수집기나 광고 활성화, 외부147·상위21·83 해결을 포함하지 않는다.

PR CI·정확 head·최종 리뷰/병합 상태는 GitHub PR과 로컬 당일 review에 기록한다.
