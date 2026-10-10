# #89 본인 저장 결과 웹 확인 검증

M3 #18, FR11/14/16·NFR01/02/05/06/08·TS15/16/20/21/26/31/35/36. 선행82/PR88 develop cdd700e 병합·종료 후 [ADR0040](../adr/0040-personal-result-web.md)부터 반영했다. 본 문서의 통과와 미실행 상태를 구분한다.

## 구현과 Red→Green

- strict 최소 decoder: 정상 저장 hash/서버 completed를 그대로 수용해야 하는 테스트가 stub malformed로 실패했다. 버전/정규 UUID·동일 match·정확한 키/최대 정수·통계 범위와 전후 proof HTTP를 연결했다. 새 HTTP 정상200·취소·uniform404도 stub unavailable로 Red를 확인했다.
- controller: 현재 메모리 matched ID·현재 account/revision, 명시 조회 동시1. start/dispose/교체/offline·WS 종료 우선·역순/늦은 응답·잘못된 match·404/일시 불가·권한 상실을 검사했다. UI 조회 버튼 미존재 Red 후 최소 본인 카드/8언어를 연결했다. 보드/상대/분모/현재 rules·광고/이벤트·일반 storage는 추가하지 않았다.
- 초기 controller/HTTP/decoder/기존 회귀41개는 통과했으나 부분 실행의 전체 coverage gate는23.14%로 실패했다. 전체 웹 실행은195개/35파일27.13초·line2273/2586=87.89% 통과했다. 이후 역순 슬롯 재사용/부분 body 취소2개를 추가했고 최종 scripts/check.ps1의 전체 웹 테스트와 coverage도 통과했다. mock 타입 추론 오류는 as const로 수정했으며 행동 Red로 계산하지 않는다.
- fixture의 UUID serde 없는 Path 추출 컴파일 오류를 String→UUID 파싱으로 수정했다. 첫 HTTPS 실패 trace는 상대가 preparing인 동안 fixture clock3000ms를 먼저 진행해 idle로 돌아간 순서였다. 양쪽9셀 준비 후 전진하도록 수정했으며 timeout/retry는 바꾸지 않았다. 최초 새 HTTPS3개37.2초/retry0 통과는 terminal close까지 유실시키는 제한된 조건이었다.

## correctness 리뷰에서 확인한 정상 종료 오분류

기준 develop cdd700e, 변경 범위와 의존 flow, pm-ai-shipping:code-review correctness를 순차 적용했다. actor→AuthorityRegistry.release→WS close→AuthPort와 결과 HTTP→post-proof→controller commit을 함께 확인했다.

P2: actor 정상 정리도 lease.closed=true와 최종 with_authority 실패로1008을 내보내 웹이 유효한 세션과 메모리 match를 제거했다. 독립 실제 WS Red는 기대Normal/실제Policy로 실패했다. UI에서 close를 전달한 추가 실행도 조회 버튼 진행이 막혔고 cleanup timeout으로 종료됐으므로 성공으로 계산하지 않는다. registry lock 안의 현재 lease release만 정상 종료로 표시하고 transport1000으로 분류했다. 교체/만료/logout/삭제 선행 release는 재분류하지 않으며 권한을 복구하지 않는다. release 뒤 철회도 독립 HTTP session/proof가 차단한다. 실제 WS전체12개와 정상 release/교체·만료·철회 경합 unit2개 통과했다.

최종 HTTPS는 close를 숨기지 않고 실제 actor 정리를 전달한다. lost saved frame, 실제 registry404, 본인 PG 결과200, 타 계정/없는 match uniform404, 503/404 안내, 응답 캡처 후 실제 logout/post-proof, 8언어·키보드·PC/mobile·storage0를 검증했다. 해당3개35.5초/retry0 통과 후 최종 전체 브라우저88개2.5분/retry0 통과했다.

## 최종 로컬 검증과 리뷰

scripts/check.ps1 exit0: fmt/all-target Clippy·workspace native/WASM·생성 타입/fixture/token 동기화·웹 format/lint/typecheck·전체 unit/coverage·fresh production build·실제 PG17.4/HTTPS 브라우저88개·문서139개/Mermaid34개 실패0. 최종 웹 제품 line2273/2586=87.89%로 gate를 통과했다. #83의 두 탭 offline 회귀도 이번 실행에서 통과했지만 원인은 미확정 OPEN으로 유지한다.

실제 PG17.4 전체30개/ignored0/3.01초와 HTTP13개/0.99초를 포함한 서버 llvm-cov exit0, check_auth_coverage.py 통과. 제품 line은 Auth385/387=99.48%, Match287/289=99.31%, Lobby362/368=98.37%, Result53/53=100%, 전체6147/6626=92.77%다. 테스트 경로를 제외한 분모이며 목표 domain95%/전체80%를 낮추지 않았다.

correctness 최종 검토 범위는 cdd700e 대비 변경과 연결된 actor/authority/WS→Auth, HTTP proof→strict decoder→controller→최소 화면이다. 서버 완료/completed·저장 hash를 웹 제안/현재 rules로 대체하지 않는 경계와 요청 identity/generation·WS/HTTP 역순 완료·start/dispose/권한 교체·취소/응답 body·정상 release와 실제 철회를 소스 및 테스트로 확인했다. P2 정상 종료 오분류를 수정했고 검토한 범위에서 지원되는 미해결 결함은 없다. 성능·보안 전체 감사는 이번 correctness 검토 범위가 아니다. CI·PR·병합 상태는 원격 확인 뒤 기록한다.

## 한계

일반 저장소에 match를 보관하지 않으므로 새로고침 후 ID 발견은 제공하지 않는다. 영속 active journal/재시작 abort는 후속18, 실제 OAuth42·장비26·사람27은 별도이며83 원인은 미확정 OPEN이다. 버그 없음·운영 성능·법적 준수·전체 개발 완료를 주장하지 않는다.
