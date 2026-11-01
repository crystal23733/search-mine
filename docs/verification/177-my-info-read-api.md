# #177 · 내 정보 요약·전적·현재 동의 조회 API

FR01/09/11/13/16·NFR01/02/05/07/08·TS17/19/20/21/22/24/29/34/35/36 → [ADR0081](../adr/0081-my-info-and-export-removal.md)·[ADR0082](../adr/0082-self-only-me-read-api.md).

선행 #176/PR179와 #174/PR175의 병합·CLOSED, develop52f51c9 clean을 확인하고 계약·추적표·시나리오를 코드보다 먼저 작성했다. 사용자 요청에 따라 일반 본인 조회는 닉네임/최근 재인증이 없어도 현재 세션으로 이용한다. 민감 변경의 기존 재인증은 유지한다.

## 행동 검증

- 닫힌 wire 입력은 실제 Red1 → Green1, 전체 필터 요약·21개 페이지·동의 view는 Red3 → Green3을 실행했다. 구현 전 PG 조회1과 HTTP3도 각각 unavailable/503의 행동 실패를 확인한 뒤 통과했다. 컴파일/테스트 fixture 작성 오류는 행동 Red로 세지 않았다.
- 도메인·실제 worker8, HTTP6을 로컬 통과했다. summary는 전체 기간/선택 필터 기준이고 승률 분모는 win/loss/draw다. abort/cancel, 0표본, null 통계, 상대 종류 확인 불가, 정확90일/소수 시각 순서, cursor 범위/필터 오염을 확인했다.
- 실제 PG18에서 본인/타인·세션 교체/삭제·계정 삭제·재인증300초 경과 허용·닉네임 없음·잠금 대기 후 시각 재검사·2초/취소·SQL 실패·10001개 차단/10000개 전체 요약·accepted daily receipt/verified time·현재/만료 동의를 검증했다. 전체 PG164 통과 후 미래/오염 행 검사를 추가했으며 최신 전체 수치는 CI에서 다시 확인한다.
- 실제 HTTPS fixture는 main과 같은 PG/router/공유 CPU worker/세션 invalidator를 사용한다. Chromium의 새 조회 API 시나리오1 통과(6.3초): 301초 후 일반 본인 조회200, no-store, 빈 전적과 null 승률, CMP 미설정, 계정 주입/잘못된 필터400을 확인했다. provider 응답은 기존 테스트 대체이며 실제 OAuth/CMP 연결 성공을 뜻하지 않는다.

## 검사와 리뷰

로컬 Rust workspace·clippy·wasm target 검사, Rust→TS 생성, Yarn4 typecheck/lint/format/build, 문서223개/시나리오36개·Mermaid35개 오류0을 실행했다. 최초 제한된 sandbox의 전체 테스트는 기존 lobby HTTP의 loopback bind 권한 때문에 실패했고, 로컬 바인딩이 허용된 실행에서 전체 통과를 확인했다. PowerShell 스크립트 자체와 로컬 coverage 계측은 실행하지 않았다. 최신 소스의 CI coverage/전체 browser/최종 head 결과는 PR에서 확인한다.

실제 코드 리뷰는 본인 식별을 SQL/세션에서만 결정하는지, 원천행 검증·전 기간 요약·저장 정렬·유효 기한·account→session 잠금·전후 세션/철회 guard·2초/동시/CPU/rate/64KiB 응답 상한을 대조했다. 원천 조회에서 board/secret seed/raw replay/상대 식별자/credential을 선택하지 않는다. 의존성·새 개인정보·설정 동기화·수집은 추가하지 않았다.

새 Me domain을95% gate에 포함했다. 현재 파일 기능은 #178 새 화면 전환과 같은 PR에서 제거한다. 이 작업은 새 UI·파일 제거·실제 CMP·운영 admin·전용403/404 화면이나 #83 해결 완료가 아니다.
