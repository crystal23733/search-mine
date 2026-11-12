# ADR0105: 현재 source 원장과 인증된 checkpoint 비교

- 상태: 승인된 FR01/16·복구/최소 권리 설계의 구현 결정
- M5 #229 · 선행 #227/PR228 병합·종료 · 상위 #25
- FR01/14/16 · NFR01/02/05/07/08 · TS20/21/28/29/31/34/35

## 결정

암호화 checkpoint의 무결성과 현재 원장과의 일치를 구분한다. Rust Snapshot 비교는 양쪽의 기존 version/source/capture/원래 expiry/한도를 검증하고 checkpoint source가 같고 capture가 source 관측보다 미래가 아닌지 확인한다. source 관측 시각에 아직 유효한 최소 account/identity 기록의 kind/id/deleted_at/expires_at 집합을 순서와 무관하게 정확히 비교한다. 누락/추가/동일 개수의 다른 기록·원래 시각 변경은 실패하고, 관측 때 이미 만료된 checkpoint 항목은 재생성하지 않는다. 원래 capture/기한을 수정하지 않는다.

native `recovery compare-snapshot`은 기존 owner capture의 read-only/repeatable-read transaction과 같은 active gate·DB/schema/source binding·500개/128KiB·2초·단조 Clock 검증을 공유한다. 같은 읽기 경로 안에서 비교하고 오류면 읽기 transaction을 종료한다. source/대상/원장에 쓰지 않으며 성공은 captured_at(원래 checkpoint), observed_at(source snapshot capture 시각), entries(그 관측에 유효한 항목 수)만 가진다. 불일치의 개인 ID·차이 목록/secret URL은 출력하지 않는다. source가 없거나 격리/출처 불명/한도·Clock·연결 오류일 때 일치를 추측하지 않는다. live main journal owner와 읽기를 병행할 수 있고 그 owner를 빼앗지 않는다.

`ledger audit`는 저장소의 같은 lock 아래 전체 index/모든 현재 개인 파일 인증 및 native 형식 검사 후에만 source 비교를 요청한다. 인증 실패는 source DB 호출 전에 차단한다. 명시적인 ledger-audit profile은 source owner URL+private identity와 DB 내부망만 가진다. public recipient/target owner를 요구하지 않고 기본 배포에서 자동 실행하지 않는다. 실제 source와 target URL을 구분하며 격리된 복구 DB에 대한 비교를 정상 원본의 일치로 간주하지 않는다. 기존 network-none verify와 target apply는 유지한다.

결과 status는 matched이며 ready/complete/activate로 표현하지 않는다. 관측 시각은 일관된 DB snapshot의 제어 메타데이터이며 검사 뒤 발생한 쓰기까지 보장하지 않는다. 검사 결과를 영구 승인 파일로 저장하거나 자동 release와 연결하지 않는다. writer 정지/최종 cutover·모든 권리 변경의 동기 내구/최신 완전성·서비스 release·실제 매체/복제본/주기/이전 이미지/RPO·RTO는 #25 후속이다. 사용자 웹 다운로드/API/화면과 기존 권리 삭제 commit 의미는 변경하지 않는다.

## 인수 시나리오

1. 같은 source의 순서가 다른 완전한 원장은 일치하고 원래 capture와 관측 시각을 따로 보존한다. 이미 만료한 항목은 제외하며 빈 집합도 명시적으로 비교한다.
2. 누락/추가/같은 개수의 다른 ID·kind·원래 시각·source/미래 capture·Clock 오류는 일치를 반환하지 않는다. 개인 차이를 노출하지 않는다.
3. 실제 PG의 같은 MVCC 관측에서 commit된 두 종류를 비교하며 미확정 변경을 섞지 않는다. live owner와 병행해도 상태를 쓰지 않고 격리/출처 누락/한도·기한 오류는 거절한다.
4. 암호화 파일의 인증 오류면 compare 호출도 하지 않는다. 실제 age/PG에서 old verify 성공→새 unlink commit→audit 실패→재캡처 audit 성공을 확인하고, 양쪽 DB 상태·복구 quarantine은 유지한다.
