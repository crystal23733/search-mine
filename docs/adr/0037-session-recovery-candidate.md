# ADR0037: 연결 장애의 비권위 세션 복구 후보

2026-10-09 · 승인된 FR01/14의 구현 구체화 · #78 · 선행 #76/PR77/developc25181e3 병합·종료 후 구현한다.

## 후보와 권한의 분리

브라우저 offline과 명확한 HTTP 연결/timeout 실패는 기존 온라인 요청을 즉시 abort하고 CSRF·공개 account/identities를 제거한다. connected는false이고 execute/계정 쓰기는 불가하다. 다만 직전 정상 닉네임 계정의 accountId·private session UUID·내부 revision·authority key를 메모리의 단일 비권위 복구 후보에 최대30초 보존한다. recoveryOwner는 accountId/revision 복사본만 제공하며 후보 자체는 요청 권한이 아니다. 무계정·닉네임 없음·쓰기 중·기타 불안정 상태에서는 후보를 만들지 않는다. 후보는 local/IndexedDB/SW/cache/URL/로그에 저장하지 않는다.

후보가 살아 있는 동안 revision은 원래 identity의 수명을 표시하며 네트워크 가용성의 증명이 아니다. 요청 generation은 중단마다 바뀌고 현재 authority는null이므로 이전 요청/응답은 성공할 수 없다. 후보를 버리거나 실제 account/session이 달라지면 revision이 바뀐다. 동일한 account의 다른 session도 같은 권한으로 복구하지 않는다. ADR0022/0032의 연결 장애 시 권한 철회는 유지하고 후보만 이 예외로 보존한다.

PendingSubmissions의 이전 revision 비교만으로는 중단 뒤 같은 identity가 복원되는 동안 늦은 응답을 구분하지 못한다. SubmissionSession은 subscribe를 제공하며 flush 수명 중 account/revision/connected가 부적합했던 사건을 기억한다. 중단된 제출은 signal을 abort하고, 복구 후 늦은 accepted/expired 응답에도 후보를 지우지 않는다. 구독은 모든 종료 경로에서 해제한다. 공식 제출19의 자동 재실행 권한을 추가하지 않는다.

## 복구와 경계

resume(expected accountId/revision)는 후보와 일치하고 온라인일 때만 bootstrap→identities→bootstrap을 실행한다. 두 proof 모두 원래 account/session·유효 nickname·CSRF와 일치해야 한다. accepted account/nickname/providers는 마지막 서버 응답을 사용한다. 성공할 때만 CSRF·계정 상태를 메모리에 다시 넣고 동일 revision을 유지한다. 후보만으로 WS/HTTP 쓰기나 입력을 보내지 않는다.

후보의 전체 예산은 최초 중단부터 단조 시계30초다. 반복 offline/online·복구 실패·resume 호출로 연장하지 않는다. 동시 resume는1개, 각 시도는10초이며 전체 기한이 먼저 끝나면 중단한다. transport가 abort를 무시해도 Promise race로 시도를 종료하고 각 await 뒤 후보 identity/시도/online/기한을 대조한다. 만료·명시 invalidate·refresh·계정 쓰기·dispose와 peer invalidation은 후보를 폐기하고 늦은 완료를 버린다.

HTTP 어댑터는 fetch 거절/읽기 중단/timeout을 AuthConnectionError로 구분한다. 브라우저는 fetch 거절의 네트워크/CORS/redirect 원인을 구분하지 않으므로 이 분류만으로 네트워크 장애가 확정됐다고 보지 않는다. 후보에 권한이 없고 복구에도 유효한 fresh proof가 필수다. 수신한 잘못된 DTO/JSON/redirect 응답·권한 불일치/401 등은 일반 AuthError이며 후보를 즉시 폐기한다. 어댑터 밖 원인을 모르는 예외는 연결 실패로 추정하지 않는다. AuthenticatedRequests의 pre/post proof가 분류된 전송 실패인 경우만 suspend로 전달한다. 대상 작업의 domain 오류는 기존처럼 원형을 보존하며 자동 재실행하지 않는다. 일시적 연결 실패 중에도 공개 계정과 권한은 복원하지 않는다.

브라우저 offline은 suspend, online/visible/pageshow는 후보가 있으면 resume, 없으면 기존 refresh를 사용한다. 다른 탭 logout/삭제 알림은 invalidate 후 refresh한다. 이 단계의 화면은 연결 장애 안내를 유지하며 자동 WS 복구·화면/명령 유지가 완료됐다고 표시하지 않는다. 다음 #18 하위 작업은 recoveryOwner로 원래 controller 수명만 식별하고, 성공한 resume 뒤에만 WS backoff/재전송을 수행한다.

## 검증과 제한

TS15/16/20/31/35/36: 후보 중 요청/쓰기0·원래 revision 유지/폐기 증가·같은 계정 다른 session/전후 proof 교체·일시적 실패·정확30초/개별10초·동시1·역순/abort 무시·peer invalidation/refresh/dispose·무계정·공개 DTO 비밀값 없음을 행동 TDD로 확인한다. 실제 PostgreSQL/HTTPS cookie에서 PC/mobile의 offline→권한 차단→동일 session 복구와 offline 중 logout→복구 거절을 확인한다. 30초는 로컬 시도 예산이고 서버가 감지한 grace/승패가 아니다. 실제 제공자42·장비26·사람27·영속 재시작/결과 재조회는 별도다.
