# 0042. 비정상 종료 결과의 미확인 통계

상태: 채택 · M3 #18/#93 · FR11/14/16, NFR01/02/05/06/07/08 · TS15/16/20/21/26/31/35/36. 선행91/PR92 develop8347efa 병합·종료 확인.

현재 최종 actor 결과는 elapsed와 모든 통계가 알려져 있다. 서버가 강제 종료된 뒤에는 종료 직전 명령·최종 통계를 알 수 없고 매 클릭의 영속 로그를 추가하지 않는 기존 DB 설계를 유지한다. 후속 startup recovery의 server_failure/abort에는 end_elapsed_ms:null, own:null을 사용한다. 두 필드 중 하나만 null인 조합과 server_failure/abort/completed:false 이외의 unknown 조합은 거절한다. 정상 actor 결과의 수치와 JSON은 유지하며0 또는 시작 당시 통계를 최종 통계로 쓰지 않는다. 기존 ResultRepository의 FinishedMatch는 항상 알려진 통계인 채 유지한다.

서버 StoredPersonalResult/project와 Rust PersonalResult는 Option을 사용하고 생성 TS는 number|null/ResultStats|null이다. `#[ts(type="number | null")]`은 u64 Option의 빅정수 표현을 기존 JS safe integer 계약으로 유지하며 실제 생성 출력/roundtrip 검사를 한다. 새 필드를 늘리지 않고 기존 다섯 키를 유지한다. 최소 decoder는 알려진 수치의 범위를 기존대로 검사하고 unknown의 완전한 조합을 추가 검증한다. 완료를 재계산하지 않으며 unknown에서 계약상 false여야 하는지만 검증한다. 구버전 웹은 null을 fail-closed 처리하므로 잘못된 숫자/완료를 표시하지 않는다. 공개 WS/게임 규칙/전체 프로토콜 version은 바꾸지 않고 forward migration→신 웹 배포→후속 recovery writer 활성화 순서다.

새 migration은 기존 부모 end_elapsed_ms와 자식 통계4개의 NOT NULL만 완화하며 기존 범위 CHECK/PK/계정FK cascade/90일·7일 정책을 유지한다. 부모 unknown elapsed는 server_failure에만 허용, 자식 통계는 모두 known 또는 outcome abort이며 모두 null인 경우만 허용한다. SQL CHECK의 null 통과/다른 행 참조 불가를 고려해 IS NULL/IS NOT NULL의 명시 boolean을 사용한다. 부모/자식 간 일치도 ResultReader/project가 fail-closed 검증하며 실제 writer transaction에서 원자 조립한다. 기존 정상 writer가 이미 저장된 unknown abort와 충돌하면 Malformed로 거절하고 절대 부모/참여 행을 재삽입/갱신하지 않는다. 과거 결과와 이전 migration checksum은 유지한다.

화면은 기존 server_failure/abort 제목과 saved·다시하기/홈을 유지하고 own:null이면8언어 ‘이 대전의 통계를 확인할 수 없습니다’를 표시한다. 통계 카드 안에 숫자/진행률/분모를 생성하지 않는다. 알려진 정상 결과는 그대로 표시한다. 일반 storage·현재 rules 결합·새 완료 이벤트·광고는 추가하지 않는다. [화면 매핑](../design/18-design-layout.md)의 참조10 결과 카드와14 서버 장애 의미를 따른다.

검증: decoder/실제 DB unknown 양성 Red→Green, domain/project· 최소 HTTP200 unknown 및 다른 계정404·권한 회귀; 실제 PG에서 새 migration/이전 known행 보존,16개 null패턴/다른 reason/outcome 거절,부모/자식 불일치 reader거절,writer충돌/삭제retry 비복원. strictdecoder 오염/partial/true/타reason/outcome거절,UI 실제 숫자미생성/8locale·일반storage0,actual production reader/router + PG/HTTPS PC/mobile. fixture가 SQL로 seed하는 unknown 결과는 계약 검증용이며 실제 프로세스 재시작으로 보고하지 않는다. 후속 journal/admission/startup recovery/processkill 통합과 새로고침 결과 발견이 끝나기 전 상위18을 닫지 않는다.

참조: [PostgreSQL18 제약 조건](https://www.postgresql.org/docs/18/ddl-constraints.html).
