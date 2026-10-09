# #68 검증: 온라인 화면의 세션 인증 요청 포트

2026-10-09 · M3/#17 · FR01/06/07/14, NFR01/02/05 · TS10/12/15/20/31/36 · [ADR0032](../adr/0032-session-bound-web-requests.md). 선행 #66/PR67은 developf5f9dd26에 병합·종료했다.

## 변경과 실행 근거

AuthenticatedRequests의 execute는 예상 accountId/revision과 현재 메모리 session/generation을 캡처한다. 전/후 bootstrap으로 실제 소유권을 대조하고5분 CSRF를 새로 받아 대상 transport에만 전달한다. 별도 requests 모듈이 요청 동시8개/전체10초·타이머/listener/slot·취소/late reply를 담당하고 AuthPort는 기존 begin/dispose에 abortAll을 연결한다. 안정된 조회는 working/revision/계정 안내를 바꾸지 않는다.

domain 오류는 원형 그대로 전달하고 자동 재시도하지 않는다. 검증된 proof 실패/권한 불일치·명시적 auth_required만 캡처한 권한이 여전히 현재일 때 무효화한다. caller 취소·기한·capacity·stale는 요청 오류이며 계정 권리 API를 대신하지 않는다. 이미 서버가 처리한 효과를 취소로 되돌린다고 표현하지 않는다.

실행 가능한 초기 stub에서15개 행동이 실제로 실패했다. 구현 뒤 신규15+기존10=25개가 통과했고 리뷰의 추가3개를 포함해 신규18+기존10=28개가 통과했다(4.91s). 컴파일 오류를 Red로 계산하지 않았다. 초기 stub의 즉시 거절 때문에 관측 대기 중 보고된 추가 rejection도 원본 로그와 ignored review에 기록했다.

안정된 병렬 요청의 서로 다른 fresh proof와 역순 완료·AuthState/working/revision 불변·pre/post account/session 대조·같은 계정의 session rotation·무계정/닉네임 없음/offline/폐기·caller pre-abort/대상 중 취소·refresh/invalidate/nickname/dispose·abort를 무시하는 late bootstrap/target·오래된401의 새 계정 보호·9번째 거절/취소 slot 재사용·정확9999/10000ms 전체 budget·타이머/listener 정리를 확인했다. 최초 타입/lint 검사도 통과했다. 추가 패키지/DB 필드/Rust 규칙/프로토콜 변경은 없다.

## 리뷰와 후속 출구

pm-skills code-review의 correctness/security를 scope68/basef5f9dd26에 순차 적용했다. 예상 권한→private snapshot→bootstrap decode→session-bound CSRF→대상 effect→후속 bootstrap→반환/무효화와 refresh/계정 쓰기/dispose의 generation을 조사했다. authority가 제안과 다른 계정/같은 계정의 새 session인 실행과 두 요청 역순 완료·slot 재사용을 강제로 검사했다. 과거 대상401과 초기 proof timeout·개별 target 취소/peer도 추가 관측했다. 검토 범위에서 추가로 뒷받침되는 결함은 남지 않았다.

전/후 bootstrap의 추가 read/RTT와 미니PC 용량은 실측 전이며 #26의 대상이다. 실제 로비 HTTP decoder/WS·8언어 화면/HTTPS 통합은 다음 #17 하위 작업이다. 이 포트는 아직 그 화면에 연결하지 않았으며 포트 시험 성공을 공개 온라인 UI 완료로 보고하지 않는다. 실키 #42·사람 #27도 별도다. 전체 검사·coverage·실DB/HTTPS CI의 실제 수치는 PR과 로컬 일별 review에 기록한다.

전체 검증에서 기존 local practice의 모바일 시험은256셀을 기다린 뒤 정확한 첫3초 문구를 요구하여 실패했다(47passed/1failed/2미실행). trace에는 grid 관측 완료 시3초 표시가 있으나 이어진 assertion 사이에 카운트가 진행했다. [ADR0016](../adr/0016-local-match-controller-and-training-fixture.md)대로 다른 난이도/재시작과 같은1/2/3초의 countdown 상태를 관측하도록 고쳤다. 제품의3초 규칙/controller·timeout/retry는 유지하며 봇 진행·언어별 진행 보존·재시작과 정리 oracle도 유지한다. 실패 원본은 ignored .tmp와 날짜별 review에 보존한다.

관측 수정 뒤 PC/mobile 각4회8개가58.0s에 통과했다. 최종 웹 전체110개(20.85s), lines1505/1721=87.44%와 requests53/53=100%를 확인했다. 실제 DB 없이 로컬에서 무시되는26개를 실DB 성공으로 계산하지 않는다.

scripts/check.ps1 최종 exit0: locked fmt/all-target Clippy/workspace native/WASM·types/fixture/tokens·웹 형식/lint/typecheck/110unit/build·browser50(2.5m)·docs121/Mermaid34. 최초 타입 오류·관측 실패는 성공으로 계산하지 않았다. 최신 CI의 실제 DB/HTTPS66와 필수 checks·head는 PR 및 로컬 review에서 확인하고 병합한다.
