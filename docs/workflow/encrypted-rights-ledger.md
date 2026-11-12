# 권리 원장 암호화 checkpoint

[#227](https://github.com/crystal23733/search-mine/issues/227) · [ADR0104](../adr/0104-encrypted-recovery-checkpoint.md) · [검증](../verification/227-encrypted-recovery-checkpoint.md).

## 준비와 범위

이 도구는 운영자 전용이다. 사용자 전적·계정·랭킹은 기존 웹 화면에서 처리한다. account/identity 삭제 기록의 최소 UUID·원래 시각만 메모리에서 암호화하며 개인 파일 다운로드/API/화면을 만들지 않는다. 운영 DB와 다른 제한 디렉터리를 `LIAR_LEDGER_DIR`로 지정한다. 실제 별도 매체·복제·스케줄/RPO·RTO·최신 cutover 검수는 #25에 남으며 로컬 fixture로 대체하지 않는다.

기존 [암호화 백업](encrypted-backups.md)의 age recipient/identity 파일 형식을 사용한다. `backup_recipient`는 공개 암호화 수신자 파일, `backup_identity`는 별도 보호하는 비밀키다. `migration_database_url`은 해당 원본 또는 격리 복구 대상의 owner URL 파일이다. 비밀키를 저장 job에 배치하거나 source/target URL을 혼동하지 않는다. 이미지/키/URL을 실제 운영 로그나 명령 인자로 출력하지 않는다. 전용 root는 소유자10001:10001/권한0700으로 준비하고 다른 사용자 파일을 섞지 않는다. Docker Desktop/운영 호스트의 실제 UID·매체·전원 차단 내구는 운영자가 확인한다.

`deploy/ledger.compose.yml`은 명시적 overlay이며 기본 배포는 job을 실행하지 않는다. `recovery-ledger` target을 build하고 운영이 확인한 image digest를 `LIAR_LEDGER_IMAGE`로 지정한다. 파일 secret은 `LIAR_SECRETS_DIR`를 따른다. capture/apply는 DB 내부망만, verify/prune은 network none이다. 공개 host 포트와 provider credential을 추가하지 않는다.

## 원본 캡처·인증·기한 정리

아래 source env는 현재 운영 원본을 가리키는 제한 파일이다. 기존 배포와 같은 project/network를 사용하며 `--no-deps`로 DB나 서비스를 새로 시작하지 않는다. 실제 운영 설정/계정 작업은 사용자가 한다.

```sh
docker compose -p liar --env-file /restricted/source.env -f deploy/compose.yml -f deploy/ledger.compose.yml --profile ledger-capture build ledger-capture
docker compose -p liar --env-file /restricted/source.env -f deploy/compose.yml -f deploy/ledger.compose.yml --profile ledger-capture run --rm --no-deps -T ledger-capture
```

capture는 공개 recipient만 가지며 native read-only snapshot을 읽는다. 한도500개/128KiB를 초과하면 부분 결과를 발행하지 않는다. 원래 expiry별 개인 암호문과 동일 메타데이터의 암호화 index를 발행하고 current pointer를 원자 교체한다. 파일/평문 manifest/index에는 개인 UUID를 넣지 않는다. 직전 current와 source UUID가 다르거나 capture 시각이 역행하면 거절한다. pointer commit 전 실패는 직전 current를 유지한다. commit 뒤 fsync/정리 오류는 실패로 알리고 새 current가 가리키는 세대는 보존한다. 이런 오류를 무조건 미발행으로 간주해 자료를 삭제하지 않고 다음 검증으로 실제 current를 확인한다.

```sh
docker compose -p liar --env-file /restricted/source.env -f deploy/compose.yml -f deploy/ledger.compose.yml --profile ledger-verify run --rm --no-deps -T ledger-verify
docker compose -p liar --env-file /restricted/source.env -f deploy/compose.yml -f deploy/ledger.compose.yml --profile ledger-prune run --rm --no-deps -T ledger-prune
```

verify는 비밀키만 사용해 전체 index와 현재 유효한 모든 개인 파일의 age 인증을 완료한 후 Rust `inspect-snapshot`으로 검사한다. prune은 key/DB/network 없이 원래 expiry에 도달한 개인 암호문과 식별된 이전 세대/중단 잔여만 정리한다. 양쪽 모두 전용 lock을 요구한다. 알 수 없는 파일/링크/손상 metadata는 따라가거나 임의 삭제하지 않고 실패한다. 원래 시각은 수정하지 않으며 mtime·재캡처로 보관 기한을 늘리지 않는다. capture+28일을 초과하면 개인 파일/index/세대를 제거하고 source UUID·마지막 capture 시각의 제어 pointer만 유지한다. 검증 성공은 저장 자료의 무결성과 형식 검사이며 최신 완전성의 증명은 아니다.

실제 주기 실행은 아직 자동 구성하지 않는다. 운영자는 원래 기한의 정리 주기·중단 후 재개 직전 정리·모든 복제본과 물리 매체의 만료를 검증해야 한다. 잘못된 metadata 때문에 정리가 실패한 것을 정리 완료로 계산하지 않는다. age 인증은 public recipient를 가진 송신자의 서명이 아니므로 신뢰하는 운영자 전용 저장소·키 접근 통제를 유지한다.

## 격리 복구 대상 적용

[복구 격리 절차](recovery-quarantine.md)에 따라 관리 백업을 원본과 구분한 빈 대상에 복원하고 migration/quarantine을 확인한다. gate가 없는 과거 백업을 임의 활성화하지 않는다. source UUID가 없는 백업도 추측해서 채우지 않는다. 복구 대상의 server/web/Tunnel을 시작하지 않는다. source와 구분한 target env/secret/project와 같은 checkpoint 디렉터리를 사용한다.

```sh
docker compose -p liar-recovery --env-file /restricted/restore-target.env -f deploy/compose.yml --profile recovery run --rm --no-deps -T recovery
docker compose -p liar-recovery --env-file /restricted/restore-target.env -f deploy/compose.yml -f deploy/ledger.compose.yml --profile ledger-apply run --rm --no-deps -T ledger-apply apply --incident RECOVERY_UUID
```

`RECOVERY_UUID`는 첫 명령이 반환한 해당 대상의 incident UUID로 대체한다. apply job은 target owner URL과 비밀키만 가지며, 같은 lock 안에서 모든 인증/대응/원래 시각·한도 검증을 완료한 메모리 Snapshot만 native apply-snapshot stdin으로 전달한다. `age --decrypt | recovery ...` 형태로 인증 완료 전에 SQL에 전달하지 않는다. 마지막 파일 손상·다른 키·누락·출처 불일치·Clock/기한 오류면 거절한다. native는 같은 transaction에서 권리 재적용/세션·동의 무효화를 수행하고 성공도 quarantine을 유지한다.

이 도구는 서비스 활성화 명령을 제공하지 않는다. 모든 권리 변경의 동기 내구·최신 완전성/cutover, 기존 운영 DB/이전 이미지 호환성, 실제 별도 매체/RPO·RTO 등 #25의 나머지 출구가 완료되기 전 접근을 재개하지 않는다.
