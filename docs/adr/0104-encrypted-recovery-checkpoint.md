# ADR0104: 권리 원장 암호화 checkpoint와 인증 후 적용

- 상태: 승인된 FR01/16·복구/최소 권리 설계의 구현 결정
- M5 #227 · 선행 #225/PR226 병합·종료 · 상위 #25
- FR01/14/16 · NFR01/02/05/07/08 · TS20/21/28/29/31/34/35

## 결정

owner native capture를 별도 전용 디렉터리의 암호화 checkpoint로 연결한다. 파일명/평문 manifest/index에는 계정·identity UUID를 넣지 않는다. 원래 expiry가 같은 항목을 하나의 age 파일로 묶고 이름은 원래 expiry+임의 UUID로 만든다. 공개 manifest는 version/source UUID/capture 시각/파일명·원래 expiry·크기·ciphertext SHA256만 포함하며 같은 내용을 암호화 index에도 넣어 전체 인증 뒤 일치를 검사한다. index에는 개인 항목을 넣지 않으므로 원래 기한에 개인 암호문을 지우면서 index를 다시 암호화하거나 capture 시각을 바꿀 필요가 없다. source UUID·마지막 capture 시각은 개인 식별자가 아닌 저장소 혼동/역행 방지 제어 메타데이터로 current pointer에 유지한다.

전용 regular/no-symlink root와 비차단 flock을 사용한다. snapshot은 메모리에서만 처리한다. 각 파일/manifest/디렉터리를 fsync하고 준비 세대를 rename한 뒤 current pointer를 원자 교체한다. pointer commit 전 발행 실패는 기한이 유효한 이전 current를 보존한다. commit 뒤 fsync/정리 오류는 새 current의 세대를 지우지 않고 실패로 알리며 실제 매체의 내구는 별도 검증한다. 새 발행은 새 출처/과거 capture로 바꾸지 않는다. 완성된 새 current가 확정된 뒤 이전 세대와 식별한 중단 잔여를 정리한다. 만료 정리는 원래 항목 expiry를 사용하며 mtime/재캡처로 연장하지 않는다. 만료한 개인 파일은 prune/verify/capture 시 제거하고 만료 파일을 재적용하지 않는다. 전체 capture+28일이 지나면 index/세대를 지우고 출처·마지막 시각 제어 pointer만 남긴다. 실제 호스트 주기/전원 중단 뒤 즉시 정리 검수는 #25 후속이며 자동 스케줄이 설정됐다고 주장하지 않는다. 알 수 없는 파일·링크·손상 metadata는 임의 삭제하지 않고 실패한다.

verify는 network none과 private identity만 사용한다. age index 전체 인증과 manifest 대응, 현재 미만료 모든 개인 파일의 크기/hash/전체 age 인증·동일 source/capture/expiry 대응을 마친 뒤 하나의 Snapshot을 구성한다. JSON duplicate/unknown envelope와 한도를 거절한다. Rust에 offline `inspect-snapshot`을 추가해 도메인 규칙의 단일 원천으로 최종 검증하며 DB URL/연결 없이 최소 제어 요약만 반환한다. 마지막 파일 인증/누락·다른 키·모델/Clock/한도 오류가 있으면 target SQL 명령을 호출하지 않는다. 인증 전 평문을 DB 도구에 pipe하지 않는다. apply는 같은 lock 안에서 검증된 메모리 Snapshot만 owner native apply-snapshot stdin으로 전달한다. URL은 기존 migration owner secret 파일에서 자식 환경으로만 전달하고 argv/로그에 넣지 않는다. 성공도 quarantine이며 release 명령은 없다.

명시적 Compose overlay의 capture는 owner URL+public recipient와 DB 내부망만, verify는 private identity/network none, prune은 key 없음/network none, apply는 target owner URL+private identity와 DB 내부망만 사용한다. nonroot/read-only/cap drop/호스트 포트0, 평문 파일0, 기본 배포 자동 시작0이다. 기존 backup target과 별도로 recovery-ledger image target을 제공한다. 기존 잠금/fsync/file 검증 primitive는 재사용하고 archive 정책은 분리한다.

신뢰하는 운영자 전용 저장소가 전제다. age의 인증은 ciphertext 손상/변조 검증이며 public recipient를 가진 자의 송신자 서명은 아니다. source UUID 일치·현재 pointer·capture 성공은 모든 권리 변경의 최신 영구 보관 증명이 아니다. 동기 내구/최신 완전성·cutover·서비스 release·이전 실제 이미지/매체/호스트 RPO·RTO는 #25에 유지한다. 삭제 요청의 기존 commit/실패 의미와 사용자 다운로드 없는 웹 흐름은 바꾸지 않는다.

## 인수 시나리오

1. 두 종류의 원장을 공개 recipient만으로 암호화하고 파일/manifest/로그에 개인 UUID를 남기지 않는다. 서로 다른 원래 expiry를 분리한다.
2. 원래 expiry 경계에 해당 개인 암호문만 제거하고 나머지는 원래 capture 시각으로 검증한다. mtime/재캡처가 기한을 늘리지 않는다.
3. pointer commit 전 발행/암호화/rename 실패와 중단 잔여는 기한이 유효한 이전 current를 보존한다. 다른 source/과거 capture, 링크/알 수 없는 파일/손상 metadata는 거절한다.
4. index/마지막 개인 파일의 손상·잘못된 키·누락·expiry/원본 대응 불일치는 DB 호출 전에 차단한다. 전체 인증 뒤에만 Rust 검사와 target apply를 실행한다.
5. 실제 PG18/age에서 과거 백업을 빈 DB에 복원하고 이후 원장을 적용해 삭제 계정이 남지 않으며, 실패 시 복구 DB가 바뀌지 않고 성공도 quarantine이다. job별 key/네트워크와 fixture 정리를 확인한다.
