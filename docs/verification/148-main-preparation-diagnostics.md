# #148 · 실제 main 준비·재시작 진단 보존

NFR01/02/05/07/08 · TS15/16/20/21/31. 선행145 CLOSED/PR146 병합과 최신 develop5a6f692·clean을 확인하고 [ADR0069](../adr/0069-main-preparation-diagnostics.md)를 코드보다 먼저 작성했다.

실제 child의 로그가 재시작·동시 실행 후에도 고유 경로로 보존되고, 실패 시 해당 child의 마지막 공개 상태/단조시간/stderr가 남아야 한다. 현재 pool 주소 경로 재사용과 stderr=null의 관측 유실을 Red로 확인한 뒤 최소 helper로 수정한다. 원래8초 ready/4초 pair/2초 admission/SQL 예산과 실제 core/OS를 유지한다.

실제 child 재시작 Red0pass/1fail/2.48초: 두 실행이 같은 pool 주소 로그 경로를 사용했다. 고유 create_new 경로·reader/종료 수명으로 수정한 뒤 Green1pass/1.72초. 실제 owner-loss1개/3.74초·OS 재시작4개/4.89초, all-targets clippy15.45초와 문서196개 검사를 통과했다. 새 독립 DB의 전체 PostgreSQL150개/32.78초도 통과했다.

코드 리뷰에서 reader IO 오류의 반복/빈 성공 가능성을 확인해 고정 code 기록 후 종료하고, 성공 경로의 reader 실패도 시험 실패가 되도록 보완했다. JSON은 닫힌 component/stage/code와 count만 보존하며 민감 필드·비JSON 원문은 제외한다. 기존 budget/core/OS·프로파일 환경 전달, EOF/join/child 종료, restart/동시 경로와 CI always artifact를 대조했다. 추가로 확정한 제품 결함은 없다.

전체/fresh·반복 관측·최신6CI/정확head 병합 결과는 실제 수행 뒤 PR과 당일 review에 기록한다. #141 전체 원인은 아직 미확정이며 진단 보존 완료만으로 닫지 않는다. 원격 성공·실패 JSONL artifact도 실제 다운로드해 내용과 비밀 제외를 확인한다.
