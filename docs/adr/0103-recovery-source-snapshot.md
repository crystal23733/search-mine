# ADR0103: 권리 원장의 일관된 캡처와 출처 검증

- 상태: 승인된 FR01/16·복구/최소 권리 설계의 구현 결정
- M5 #225 · 선행 #217/PR218·#83/PR224 병합·종료 · 상위 #25
- FR01/14/16 · NFR01/02/05/07/08 · TS20/21/28/29/31/34/35

## 결정

복구 incident UUID와 별도로 owner 전용 `service_recovery_origin` singleton에 source UUID를 보존한다. 관리 dump에 출처 행을 포함한다. 새 migration은 같은 DB/schema에 바인딩된 active gate의 UUID만 최초 출처로 채택한다. factory-empty 초기화는 active gate와 출처를 같은 transaction으로 만든다. 출처 누락/격리/출처를 도입하기 전 백업은 자동 추측하지 않는다. 출처는 activation 허가가 아니며 serving 권한을 추가하지 않는다.

owner CLI `recovery capture`는 같은 repeatable-read/read-only transaction에서 active gate·현재 binding·출처와 계정 삭제/연결 해제 tombstone을 읽는다. live main을 중지하거나 journal owner를 가져오지 않는다. DB에서 확정된 MVCC snapshot만 읽으며 삭제 요청의 atomic commit 의미는 변경하지 않는다. capture 시점의 미만료 항목을 두 종류 합계501개까지 조회하고500개 초과면 전체 실패한다. 원래 정수 초 deleted_at/정확2419200초 expires_at을 보존하며 잘못된 시각을 반올림·수정하지 않는다. 전체2초/128KiB 한도와 Clock 역행/미래 검증을 적용한다.

출력 snapshot은 version1/source_id/captured_at/entries만 가진다. 항목은 기존 최소 kind/id/deleted_at/expires_at이다. 기존 수동 Ledger의 recovery_id/5분 계약은 유지한다. Snapshot은 원래 capture 시각을 최대28일 검증하고 당시 미만료였던 각 항목의 원래 만료를 검사한다. 오래된 snapshot에 현재 capture 시각을 덮어쓰지 않는다. `recovery apply-snapshot <incident UUID>`는 인증 완료된 snapshot만 stdin으로 받아 현재 quarantine gate·DB/schema·incident와 백업에 보존된 source UUID를 같은 transaction에서 확인한다. 출처가 없거나 다르면 rollback한다. 현재 이미 만료된 항목은 재적용하지 않고 현재 미만료 항목만 기존 공통 삭제 절차로 처리한다. 적용 도중 항목이 만료되면 rollback한다. 세션/거래/동의/선택 raw·과거 Apple credentials/queue/receipt 무효화와 coverage 보정은 기존 apply와 공유한다. 성공해도 quarantine을 유지한다.

native stdout은 운영용 민감 데이터이며 파일/로그/웹으로 공개하지 않는다. 후속 인증 암호화 계층만 이를 소비한다. 이 하위 단계 자체는 독립 내구 보관·최신 완전성의 증거가 아니다. 독립 암호화 저장/원래 항목별 expiry 정리·인증 완료 뒤 SQL 전달·모든 권리 변경의 cutover·release 및 실제 매체/RPO·RTO/이전 이미지 호환성은 #25 후속으로 유지한다. 사용자 웹의 다운로드/API/화면과 제품 권리 삭제의 commit/실패 의미는 바꾸지 않는다.

## 인수 시나리오

1. 두 종류의 확정 원장을 원래 시각으로 함께 캡처하고 아직 commit하지 않은 변경은 포함하지 않는다. 빈 snapshot은 명시적 빈 결과다.
2. source UUID는 정상 업그레이드/빈 factory에서 생성되고 quarantine 재실행에도 변하지 않는다. 격리/잘못된 binding/복구 원본 불명에서는 자동 생성하거나 capture하지 않는다.
3. 500개/128KiB·미래/역행/잘못된 초·unknown/nil/중복/원래 expiry 불일치는 전체 실패하며 부분 snapshot을 출력하지 않는다.
4. 다른 출처/누락 출처/다른 incident/active gate에서 적용은 실패한다. 같은 출처의 원래 capture는5분 뒤에도 보존되고 만료 항목은 삭제를 재생성하지 않는다.
5. 같은 snapshot의 재적용은 멱등이며 복구 데이터 삭제·세션/동의 무효화는 원자적이다. DB 오류/Clock/기한/owner 경합은 rollback, gate는 격리 상태다.
