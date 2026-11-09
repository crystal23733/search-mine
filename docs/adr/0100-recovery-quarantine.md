# ADR0100: 복구 DB 기본 격리와 권리 원장 재적용

- 상태: 승인된 FR01/16·복구/최소 권리 설계의 구현 결정
- M5 #217 · 선행 #215/PR216 병합·종료 · 상위 #25
- FR01/14/16 · NFR01/02/05/07/08 · TS20/21/28/29/31/34/35

## 결정

관리 백업에서 `service_recovery_control`의 DATA를 제외한다. 이 owner 전용 singleton은 현재 database/schema OID, active/quarantined 상태, 임의 recovery UUID, 갱신 시각만 가진다. 개인 ID/자격 증명/백업 키는 넣지 않는다. main은 DB를 쓰면 HTTP bind/인증 작업 전에 active+동일 DB/schema를 확인하고 journal owner 획득 뒤 다시 확인한다. 복구/운영 명령은 같은 journal advisory key의 transaction lock을 얻어야 하므로 살아 있는 main과 경합하면 조작하지 않는다. readiness도 gate를 검사한다. 이 절차는 관리 dump와 별도 빈 복구 대상을 전제로 하며 임의 DBA의 gate 복사/DDL 우회를 방어한다고 주장하지 않는다.

migration 바이너리는 시작 전에 실제 빈 schema임을 확인한 proof를 캡처하고 migration 뒤에도 모든 제품 테이블이 빈 경우에만 최초 active를 만든다. 이미 데이터가 있는 기존/복구 DB, gate 누락/격리 상태는 재실행으로 자동 활성화하지 않는다. 기존 실제 운영 DB의 최초 도입과 안전한 release 절차는 #25의 별도 검증이다. 직접 migrate하는 통합 fixture도 명시적인 빈-schema proof를 사용한다. serving은 gate SELECT만, migration owner만 gate를 수정한다.

계정 삭제는 기존 account tombstone을 사용한다. 연결 해제/Apple 연결 철회에서는 기존 identity UUID와 원래 삭제 시각/정확2419200초 expiry만 새 `auth_identity_tombstones`에 남긴다. 원래28일 보존 목적을 넓히거나 provider subject/email 등 새 개인정보를 저장하지 않는다. 계정 전체 삭제에 따른 identity cascade는 account tombstone이 덮으므로 중복 identity 원장을 만들지 않는다. 기존60초 auth cleanup이 만료 identity tombstone을 정리한다.

owner 전용 recovery CLI는 quarantine과 apply만 제공한다. quarantine은 현재 대상에 recovery UUID를 만들고 apply는 그 UUID/DB/schema 바인딩을 확인한다. 입력 JSON은 version/captured_at/recovery_id/entries만 허용한다. account 또는 identity의 최소 ID·deleted_at·expires_at만 최대500개/128KiB, capture는 현재보다 미래가 아니며5분 이내, nil/중복/unknown field·미래 삭제/정확28일 불일치/이미 만료된 항목은 전부 거절한다. 입력 전체 검증 뒤 한 transaction에서 tombstone을 멱등 삽입하고 계정/연결을 삭제한다.

동시에 복구된 모든 세션/거래를 제거해 동의·선택 raw를 cascade하고, 오래된 Apple credentials/철회 queue/notification receipt를 지워 과거 자격 증명으로 새 제공자 상태를 변경하지 않는다. 실제 제공자 요청은 보내지 않는다. metric collection 시작은 보존하고 coverage_since를 현재 검증 시각 이상으로 보수적으로 전진시켜 복구 간극을 완전한0으로 집계하지 않는다. 원래 익명 expiry는 수정하지 않는다. Clock/DB/잠금/입력 오류는 전체 rollback,2초 예산과 transaction-local timeout을 사용한다. **적용 성공 후에도 quarantine을 유지하며 활성화 명령은 제공하지 않는다.**

CLI 입력은 신뢰하는 운영자가 검증한 원장이다. 이 입력 검사는 완전성/최신성의 증명이 아니다. age의 인증이 끝나기 전에 평문을 SQL 도구로 pipe하지 않는다. 독립 내구 원장의 암호화 캡처/인증 전달·모든 권리 변경의 완전한 cutover·보존 정리/호환 검증·release 승인 증거가 준비되기 전 서비스는 열지 않는다. 이 후속 출구는 #25에 남긴다. 사용자 개인 파일 다운로드/API/화면은 추가하지 않는다.

## 검증

실PG의 fresh/기존 데이터/복구 gate·binding·main TCP 접근 차단·owner 경합과 실제 삭제/연결해제·session/consent/선택 raw·Apple ciphertext/queue·coverage/익명 expiry·멱등/rollback을 확인한다. 실제 관리 암호화 dump/빈 DB restore에서 active gate 행이 복사되지 않는지 검사한다. model95/whole80·기존 전체 CI·문서/Mermaid·실제 리뷰 뒤 순차 병합한다. 실제 별도 매체/호스트/키/RPO·RTO를 테스트 fixture 성공으로 대체하지 않는다.
