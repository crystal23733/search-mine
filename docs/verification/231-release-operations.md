# #231 배포 운영 도구 검증

요구: FR16/NFR03/NFR04/NFR05/NFR07. 설계: [ADR0106](../adr/0106-controlled-recovery-resume.md). 선행 #229 완료. 상위 #24/#25/#26.

## 인수 시나리오

| ID | 관측 결과 |
|---|---|
| OPS231-01 | 원본 writer 실행 중/누락된 최신 권리/다른 source는 재개를 거절하고 대상을 quarantine으로 유지한다. |
| OPS231-02 | writer 중지 후 같은 출처 최신 원장으로 원본 sealed→대상 삭제·보존 정리→active 전환하며 원본은 serving/일반 quarantine을 거절한다. |
| OPS231-03 | 원본 sealed 이후 대상 incident 오류/정리 실패는 대상을 차단하고 같은 원본으로 재시도 가능하다. 미인증 암호문은 DB 연결 전에 실패한다. |
| OPS231-04 | 실제 인증/매칭/WS 부하를 유한 시간·유한 수로 실행하고 성공·거절·오류·지연 표본을 분리한다. 표본 부족/실패를 안정 용량으로 계산하지 않는다. |
| OPS231-05 | 번들 gzip과 실제 실행 환경을 보고하고 확인한 안정 용량의70% 이하만 admission 후보로 제시한다. |
| OPS231-06 | 배포 점검은 잘못된 origin/가변 이미지/유출 가능한 secret/host port/누락된 운영 검수를 실패로 보고하며 비밀값은 출력하지 않는다. |

## 실행 결과

- Red: 새 재개 시나리오 실행에서 미구현 `resume_from_source` E0425를 실제 확인했다. Green: 실제 PostgreSQL에서 원본 fence/최신 누락/실행 중 owner/중간 실패 재시도/보존 정리/진행 중 권리 transaction 경합5개를 검증했다. 재개 대상은 실제 main/readiness까지 확인했다.
- 전체 PostgreSQL 회귀231개 PASS(추가 cutover 경합 포함 최종5개 targeted PASS). 새 trigger의 gate SELECT를 기존 최소 DML 역할 fixture에도 맞췄으며 production serving-grants의 권한을 늘리지 않았다. CI의 동일 최종head 전체/coverage 결과는 PR에 별도로 기록한다.
- 실제 Docker/age/복원 DB smoke PASS: 최종 암호문 손상은 SQL 전 실패, 일반 apply 두 번은 quarantine, 명시 resume만 원본sealed/대상active, 이미active인 대상 재개 거절, 원래 expiry/생성물 정리. 최종 로컬 summary `.tmp/liar-smoke-dd45c281db-eunk4guz/summary.json`. 신규 migration의 gate 보호도 포함한 이미지다.
- Python 운영 계약42개 PASS. 배포 증거 누락/미래·오래된 시각/다른 이미지·limit/공개 포트/가변 이미지/비밀 출력·파일 권한/생성 실패 후 prune·오래된 receipt·불충분/중복 부하 표본을 차단한다.
- Rust workspace 단위/HTTP·WS 검사, fmt/clippy/WASM 컴파일 PASS. 기본 sandbox의 loopback bind PermissionDenied는 실행 권한을 확보해 다시 검사했다. 실행하지 않은 로컬 coverage는 달성으로 계산하지 않는다.
- 현 web build manifest: 초기 static import JS gzip합계60,194바이트(200KiB 예산 이내), lazy JS52개와 WASM512,543바이트/gzip186,330바이트를 따로 보고했다. web 코드는 이 이슈에서 변경하지 않았다.
- 실제 main release/개발 PC4판8명60초 측정 수행. 첫 관측은2,764 ACK/적용공격4/거절2,608/오류0으로, 거절 비중이 큰 workload를 일반 입력 성능으로 제시하지 않도록 적용 ACK·공격 지연/연결시간을 분리하고 고정 open/flag/attack 혼합 workload로 보완했다. 최종 혼합 관측은2,185 ACK(적용2,157/공격14/거절28), 오류0, 연결 합계480.03 player-seconds, 전체/적용ACK p95 1.84ms·공격p95 1.06ms다. 단일/dirty 개발PC 관측이며 운영 내부 입력 지연이나 미니 PC 성능이 아니다. dirty/단일/개발 장비 결과이므로 admission 후보로 채택하지 않는다.
- 문서280개·Mermaid34개 PASS. 실제 운영 장비/자원 제한/네트워크/계정/물리 매체/이전 배포 이미지/RPO·RTO·human 검수는 미실행이며 상위 이슈를 닫지 않는다.
