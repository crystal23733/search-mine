# #198 · 재인증과 지연된 세션 조회 응답

[ADR0092](../adr/0092-auth-bootstrap-cookie-ownership.md) · FR01/16·NFR01/02/05/08·TS20/29/34/35.

선행199/PR200 MERGED/CLOSED·developc611ad5 clean/origin0:0 확인. 원격 PR197 trace에는 재인증 복귀 중 중단된 bootstrap과 후속 비로그인 상태가 있다. 중단 응답의 Cookie 쓰기 전체가 관측된 것은 아니므로 유일한 자연 원인을 단정하지 않는다. 읽기 응답의 세션cookie 삭제가 최신 발급과 경합할 수 있는 코드를 확인해 같은 손실을 통제된 HTTPS/PG 순서로 재현한다. 제품 코드보다 먼저 설계/시나리오를 기록했다.

실제 검증 결과는 아래와 같다. 원격 coverage·전체 browser CI와 병합은 PR에서 확인한다. 원격 raw/trace는 .tmp/auto196-ci-final-*에 보존하며 timeout/retry/flags를 바꾸지 않는다.


## 실행 결과와 코드 리뷰

- Native Red: 무효 세션 bootstrap이 세션 Set-Cookie를 보냈고 새 계약 검사가 실패했다. 실제 HTTPS/PG Red: 재인증으로 새 쿠키가 발급된 뒤 이전 쿠키의 조회 응답을 실제 browser cookie jar에 적용하자 새 쿠키가 사라졌다. 로그/trace는 로컬 `.tmp/auth198-{native,browser}-red*`에 보존했다.
- Green: HTTP 인증 테스트 14개 통과. 기존 재인증 삭제와 새 경합 회귀 2개 통과 후 각각 3회, 총 6개가 11.8초에 통과했다. 무효 세션의 본인 API 401, 과거 proof와 새 쿠키의 조합 400, 현재 쿠키 보존과 새 문서의 본인 화면·새 명시 확인을 실제 HTTPS/PostgreSQL로 검사했다.
- `cargo clippy --locked --workspace --all-targets -- -D warnings`, `cargo test --locked --workspace` 통과. 실제 PostgreSQL ignored 통합 테스트 185개 통과(16.51초).
- 문서 검증 245개 Markdown/21개 제품 문서/18개 설계/36개 시나리오 추적, Mermaid 34개 통과. Yarn lint/typecheck/format, cargo fmt, diff 공백 검사 통과.
- 코드 리뷰: bootstrap만 세션 쓰기를 제거했으며 조회의 account/session 권한은 여전히 저장소 검증 결과만 사용한다. CSRF는 실제 요청 쿠키에 바인딩하므로 만료 쿠키가 남아도 정확한 proof로 새 OAuth를 시작할 수 있고 다른 쿠키와의 혼용은 거절한다. 명시 logout/삭제·최근 인증·Origin·쿠키 만료·OAuth callback 발급 코드는 변경하지 않았다. 원래 E2E는 새로운 문서 관측을 추가했으며 대기 시간/재시도/브라우저 flag는 변경하지 않았다.
- 미확인: PR197 자연 실패의 유일 원인, 실제 제공자 #42, 오프라인 #83은 이 통제 재현으로 완료 판정하지 않는다. 로컬 LLVM coverage는 실행하지 않았고 필수 원격 coverage/전체 browser 검사를 병합 조건으로 유지한다.
