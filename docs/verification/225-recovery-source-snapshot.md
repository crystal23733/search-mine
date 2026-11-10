# #225 권리 원장 캡처·출처 검증

FR01/14/16 · NFR01/02/05/07/08 · TS20/21/28/29/31/34/35 → [ADR0103](../adr/0103-recovery-source-snapshot.md).

## 시나리오와 구현

owner 전용 source UUID를 관리 dump에 보존하고 현재 복구 incident와 구분한다. 정상 active gate 업그레이드/빈 factory만 출처를 초기화하며, 격리/다른 binding/출처 없는 과거 복구 대상에 추측해서 생성하지 않는다. serving에는 출처 표 권한을 추가하지 않는다.

`capture`는 하나의 read-only/repeatable-read transaction에서 gate·출처와 두 종류의 확정된 tombstone을 읽는다. 원래 정수 삭제 시각/정확2419200초 expiry를 보존하고501개 감지·500개/128KiB·Clock·2초 예산을 검증한다. 현재 snapshot의 일관성과 이후 권리 변경의 최신 완전성은 구분한다.

`apply-snapshot <incident UUID>`는 원래 capture 시각을 유지한 엄격한 입력과 현재 gate/incident·출처를 함께 확인한다. 이미 만료된 항목은 재적용하지 않고, 아직 유효한 항목은 기존 공통 삭제·세션/거래/동의/선택 raw·Apple 자격 무효화와 한 transaction에서 처리한다. 대기 중 만료/오류는 전체 rollback이며 성공도 quarantine이다. 기존 수동 Ledger의5분 검증은 유지한다. native 운영용 stdout은 민감 UUID를 포함하며 사용자 웹 다운로드/API/화면을 추가하지 않는다.

## 실행 기록

설계/시나리오를 코드 전에 작성했다. 새 model/port 테스트의 초기 Red는 미구현 Snapshot 타입·capture/apply_snapshot 함수로 인한 컴파일 실패였으며, 구현 후 model7개 및 신규 실PG10개를 통과했다. 이 Red를 동작 assertion 실패로 보고하지 않는다.

실제 PG의10개 시나리오는 최소 두 종류/미확정 제외·원래 시각, gate 잠금 사이 commit된 두 표를 같은 MVCC 시점으로 읽기, 한도/미래/소수 초/출처 누락 거절, source/incident 불일치, 원래 expiry와 이미 만료된 기록 제외, 오류 rollback/멱등, 정상 upgrade와 격리/누락 binding, 실제 CLI 입력/고정 오류, live owner와 read-only capture/쓰기 Busy, SQL 대기 중 만료 rollback,2초 대기/Clock 역행을 확인한다.

로컬 최종 검증: Rust workspace483통과, fmt/clippy, WASM target check, Rust 원천 생성 타입 freshness 통과. 실제 PostgreSQL18 전용 tmpfs2GiB에서 전체221통과(41.83초, ignored 대상 실행·threads4), Python 백업 계약16통과. 문서272개/시나리오 trace36개·Mermaid34개 검사 통과. 로컬 llvm-cov는 설치되지 않아 coverage를 측정했다고 주장하지 않는다.

첫 전체 실행은 임시 DB tmpfs512MiB의 pg_wal ENOSPC로179통과/42실패했고 실패 로그를 보존했다. 해당 issue 전용 컨테이너만2GiB로 교체해 같은 전체 DB 검사를 완료했다. 별도 기본 sandbox의 Rust 실행은 로컬 HTTP bind PermissionDenied로 중단됐으며 허용된 환경에서 전체483통과했다. 사용자 볼륨/타 프로젝트 이미지나 실제 운영 DB는 수정하지 않았다.

관리 암호화 dump/빈 DB restore에서 source UUID 보존·gate 제외, 실제 serving 출처 SELECT/UPDATE 거절과 전체 회귀/coverage는 PR의 정확한 head에 대한 CI 결과를 확인한 뒤 리뷰에 기록한다. 아직 실행되지 않은 CI 성공을 이 문서로 주장하지 않는다.

## 남은 출구

독립 암호화 내구 저장·각 항목의 원래 만료일 정리·전체 인증 뒤 SQL 전달·모든 권리 변경의 최신 완전성/cutover·서비스 release·이전 실제 이미지/매체/호스트 RPO·RTO는 #25 후속이다. 이 native 캡처/적용 성공은 서비스 재개 허가가 아니다. 실제 OS PWA 설치/OAuth 및 실제 버전 업데이트는 #223에 남는다.
