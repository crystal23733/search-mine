# #217 복구 격리·원장 재적용 검증

FR01/14/16 · NFR01/02/05/07/08 · TS20/21/28/29/31/34/35 → [ADR0100](../adr/0100-recovery-quarantine.md).

## 시나리오

1. 빈 설치만 gate active를 초기화한다. 데이터가 있는 upgrade/복구 대상, 누락/격리/다른 DB·schema gate는 자동 활성화하지 않으며 실제 main TCP bind를 거절한다.
2. main journal owner가 살아 있으면 quarantine/apply가 실패하고 아무 행도 바꾸지 않는다. main의 gate 전/후 검사와 동일 owner key로 격리 변경/기동의 틈을 막는다. serving은 gate SELECT만 가능하다.
3. 직접 unlink 및 Apple 연결 철회는 기존 identity UUID/원래 시각만 정확28일 기록하고 반복/실패 transaction은 기간을 늘리거나 고아 원장을 남기지 않는다. 계정 삭제는 기존 account tombstone이 적용된다.
4. 올바른 복구 UUID의 account/identity 원장을 적용하면 source 삭제·FK cascade·세션/거래/동의/선택 raw·오래된 Apple 자격/철회 queue 정리가 같은 transaction에 commit된다. collection 시작/익명 expiry는 유지하고 coverage를 전진시킨다. 재실행은 멱등이고 gate는 계속 quarantined다.
5. 5분 capture/정확28일/500개/128KiB 경계·nil/중복/미래/만료/unknown field/wrong incident를 거절한다. DB 오류/Clock 역행/잠금·2초 예산은 전체 rollback한다. CLI는 고정 오류와 운영 count만 출력한다.
6. 관리 pg_dump에서 gate DATA가 제외되고 실제 암호화/검사/빈DB restore 후 active를 복원하지 않는다. 실제 main·배포/백업 CI와 model95/whole80를 확인한다.

## 기록

설계/시나리오를 코드 전에 작성했다. model 실제 Red(1통과/3실패)→4통과, 실PG 기본 Red(2실패: 격리 main HTTP 노출·apply 누락)→Green을 확인했다. 추가 owner 경합/rollback/CLI/factory proof/권리·세션/coverage 시나리오를 포함한 최종 실PG는211통과(73.55초), native Rust480통과, fmt/clippy 통과다. readiness의 기존 미마이그레이션 fixture는 정상 gate fixture로 고치고 격리503을 추가했으며 기존200/종료503·timeout/retry는 유지했다.

실제 Docker PG18.6/age1.3.2 관리 dump·암호화·전체 인증·빈 DB restore에서 gate 행0, 기존 row/FK/보존/coverage 계약·손상/오류 차단·고유 fixture 정리를 확인했다. 합성 restore0.241초는 운영 RTO가 아니다. Python 백업 계약16개 통과. 중간 실PG/백업 실패는 Docker VM ENOSPC였고 원문을 보존했다. 프로젝트 Yarn build cache로 식별한24개 기록만 정리(9.578GB) 후 같은 검사에 통과했으며 사용자 볼륨/타 프로젝트 이미지는 건드리지 않았다.

CI의 Recovery domain95/전체80·배포 실제 권한·기존 전체 회귀 결과는 PR에 기록한다. 로컬 llvm-cov는 미설치로 측정하지 않았다. 최신 원장의 완전성/별도 내구 저장·release·기존 실제 운영 DB 도입·이전 실제 image·호스트 RPO/RTO는 미실행이며 #25에 남는다.
