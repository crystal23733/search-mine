# ADR0069 · 실제 main 준비·재시작 진단 보존

- 상태: 채택 · 승인된 NFR01/02/05/07/08 · #148, 상위 #141
- 선행: #145 CLOSED·PR146/develop5a6f692 병합 확인
- 시나리오: TS15/16/20/21/31

## 확인된 문제와 범위

실제 owner-loss 시험은 child stderr를 버려 준비4초 실패의 단계를 알 수 없다. admission restart 시험은 pool 주소로 정한 같은 경로를 File::create로 재사용하므로 재시작이 이전 로그를 덮는다. #148은 이 확정된 관측 유실을 고치며 모든 과거 #141 준비 실패의 원인이 같다고 주장하지 않는다.

시험 전용 ObservedServer가 기존 서버 명령·실제 OS child와 종료 RAII를 감싼다. 실행별 PID+새 random UUID 경로를 create_new로 열고, stderr reader가 기존 JSON에 관측 단조 경과시간을 더해 즉시 기록한다. 각 child의 로그는 성공·실패·재시작 후에도 보존한다. 원래 서버 JSON을 바꾸거나 seed/정답/계정/토큰/접속 문자열/SQL을 새로 기록하지 않는다. 비JSON stderr는 원문 대신 고정 code로 표시한다.

관측 파일은 닫힌 component/stage/code·운영 count만 보존하는 allowlist를 적용한다. 실패 시 child를 종료하고 reader를 join해 남은 stderr를 저장한 다음, 그 실행의 로그와 최근 공개 상태 종류·server_time_ms를 출력한다. 로그 파일 오류·reader 실패도 고정 관측 code로 드러내며 검증 실패로 처리한다. 런타임·파일·reader 수명은 시험 helper가 소유한다. 실제 CLI/main 계측을 위해 기존 LLVM_PROFILE_FILE 전달은 유지한다. CI database job은 성공·실패 모두 main-preparation-diagnostics artifact를7일 보존한다.

## 준비와 검증

owner-loss 시험은 원래 HTTP ready 이후의 cold 준비를 우선 관측한다. restart 시험의 기존 HTTP+실제 board cache ready8초 관측과 pair4초·admission2초·SQL 경계는 유지한다. 같은 pool의 연속 실제 child와 동시 child가 서로 다른 로그를 남기고 이전 파일이 바뀌지 않는 행동 Red→Green을 확인한다. 준비 실패의 원인을 확인한 뒤 fixture 또는 제품 변경이 필요하면 근거와 설계를 먼저 보완한다.

실제 PG/native/main 반복·full·fresh coverage·문서/Mermaid·코드 리뷰·최신6checks/정확head 병합을 기록한다. 자원/CPU/DB의 동시 부하와 cold/cache 준비를 구분하고 성공 반복만으로 #141을 닫지 않는다. 제품 core/OsSeedSource를 fake 성공으로 바꾸거나 timeout/retry를 늘리지 않는다. #83·외부 #147/#42·운영 #25/#26·사람 #27은 별도다.
