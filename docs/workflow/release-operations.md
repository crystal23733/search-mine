# 배포 전 점검·주기 실행·실장비 측정

[ADR0106](../adr/0106-controlled-recovery-resume.md)·[검증231](../verification/231-release-operations.md). 키·실제 운영자 정보·도메인·장비를 대체하지 않는 읽기 전용 점검/격리 측정 도구다. 실제 공개 배포와 운영 검수 전에는 출시 완료가 아니다.

## 고정 이미지와 설정 점검

[컨테이너 절차](container-deployment.md)대로 먼저 source를 검증·빌드하고 실제 이미지ID 또는 registry digest를 기록한다. 이후 배포용 env의 server/web/PostgreSQL/Tunnel/backup/ledger 이미지를 모두 `sha256:<64자리>` 또는 `repository@sha256:<64자리>`로 고정한다. 기본 `:local`/`:latest`/메이저 tag는 개발 빌드용이며 공개 점검을 통과하지 않는다. PostgreSQL과 grant는 같은 `LIAR_POSTGRES_IMAGE`를 사용한다. 고정한 이미지는 같은 호스트에 존재하거나 검수된 registry에서 가져와야 한다. 불변 참조를 지정한 뒤 다시 `compose build`로 덮어쓰지 않는다.

운영 증거 JSON은 저장소 밖에 보관한다. version=1, origin=확정HTTPS origin, images={server:실제고정참조,web:실제고정참조}, limits는 렌더링된 server 환경의 `LIAR_ONLINE_/LIAR_LOBBY_/LIAR_BOARD_/LIAR_BOT_` 설정과 정확히 같은 문자열 사전이며, checks는 아래8개다. 각 항목은 status=`passed`, performed_at=UTC ISO8601, record=비밀을 제외한 제한된 검수 기록 위치를 가진다. 도구는 동일 origin/이미지에 대한7일 이내 실제 검수만 받는다. 샘플을 통과 결과로 복사하지 않는다.

- `oauth_four_providers`: 네 제공자의 실제 로그인·최소 권한·연결/해제/삭제·Apple 철회.
- `policies_and_cmp`: 실제 운영자/연락처·정책·8언어·Google 메시지 게시·거부/철회.
- `ads_approval_and_isolation`: AdSense 사이트/ads.txt/CSP·동의 허용/거부·게임/오류 광고0.
- `tunnel_and_network`: 실제 HTTPS/Tunnel·0 host port·집IP/DB 비공개·회선 약관·readiness.
- `backup_restore_and_previous_release`: 별도 매체·키 접근/보존·주기·실제 복구·이전 이미지/schema·RPO/RTO.
- `capacity_on_target`: 아래 반복 실측과 실제 CPU/RAM/업로드·admission70%·중단/부하 회복.
- `pwa_real_devices`: #223의 실제 OS 설치/실행/업데이트/OAuth 복귀.
- `language_and_accessibility`: 실제8언어 긴 문구·키보드·모바일 검수.

```sh
python3 deploy/preflight.py --env-file /etc/liar/runtime.env \
  --compose deploy/compose.yml --compose /etc/liar/providers.yml \
  --compose deploy/backup.compose.yml --compose deploy/ledger.compose.yml \
  --evidence /etc/liar/release-evidence.json
```

결과는 비밀값·설정 전문 없이 `ready` 또는 누락 코드만 출력한다. 네 제공자와 Tunnel·백업/원장 profile을 포함한 합성 Compose를 검사한다. 설정 파일/secret은 저장소 밖의 canonical 절대 경로·비symlink·0바이트 초과/16KiB 이하·타인 읽기/쓰기 금지·제한된 상위 디렉터리를 요구한다. 실제 컨테이너 UID별 읽기 권한과 매체가 물리적으로 분리됐는지는 운영자가 확인한다. `ready`는 입력한 운영 증거를 바탕으로 한 점검 성공이며 실제 배포 명령을 실행하거나 증거의 사실성을 대신 보증하지 않는다.

## 백업과 권리 checkpoint 주기

`deploy/maintenance.py`는 명시 job만 실행하고, 생성 실패 뒤에도 같은 저장소 prune을 시도한다. 비밀/개인 UUID/명령 stderr를 출력하지 않고 job별 성공/실패와 시각을0700 state 디렉터리에 원자 저장한다. 실패는 exit1이므로 systemd 실패를 무시하지 않는다. 동시 job은 state lock을 최대60초 기다리고 이후 실패한다. Docker 운영 권한이 있는 전용 사용자와 정상화된 host secret/별도 매체를 먼저 준비한다.

```sh
python3 deploy/maintenance.py backup --env-file /etc/liar/runtime.env --state-dir /var/lib/liar-maintenance
python3 deploy/maintenance.py ledger --env-file /etc/liar/runtime.env --state-dir /var/lib/liar-maintenance
```

Linux systemd 예시는 [service](../../deploy/systemd/liar-maintenance@.service)·[12시간 backup timer](../../deploy/systemd/liar-backup.timer)·[hourly ledger timer](../../deploy/systemd/liar-ledger.timer)다. `/opt/liar/current`, `/etc/liar/runtime.env`, 전용 `liar`/docker 그룹·0700 state 경로를 실제 호스트에 맞춰 운영자가 설치한다. 예시 경로를 그대로 실제 성공으로 표시하지 않는다. `systemctl enable --now liar-backup.timer liar-ledger.timer`, `list-timers`, `status liar-maintenance@backup.service`와 `@ledger.service`, journal 및 두 receipt의 최근 성공을 확인한다. `python3 deploy/maintenance.py status --state-dir /var/lib/liar-maintenance`는 백업24시간/원장2시간 초과·실패·누락을 exit1로 표시한다. timer는 백업을03:10/15:10, 원장을매시 실행해 재시도 여유를 둔다. 동시 job 실패는 timer 시간대를 분리해 해결하며 알림·별도 매체 분리/복제/28일 만료는 실제 운영 검수다. 이 절차의 hourly checkpoint는 동기 복제/RPO0을 의미하지 않는다.

## 실제 main·인증·매칭·WS 부하

운영 DB 대신 [README의 격리 PostgreSQL](../../README.md)을 준비하고 아래를 **미니 PC에서** 실행한다. 시험은 고유 schema/loopback 실제 main 프로세스를 만들고 종료·schema 정리한다. OAuth 외부 교환만 저장소의 제한 테스트 helper로 준비한 세션을 사용하며 운영 바이너리에 인증 우회를 넣지 않는다. bundled 규칙·랜덤 인증 보드·실제 DB/journal/매칭/WS를 사용한다. 공개 관측으로 닫힌 칸을 열고 게이지가 차면 공격한다. 거절된 명령과 실제 적용 공격/오류를 별도로 센다. k6 의존성을 추가하는 대신 기존 Rust 실제 제품 포트를 사용한다.

```sh
# 비밀 URL은 셸 이력에 입력하지 말고 격리 개발 DB의 환경으로 전달한다.
LIAR_LOAD_PAIRS=4 LIAR_LOAD_SECONDS=60 cargo test --locked --release \
  -p liar-server --test postgres isolated_main_load_measurement -- --ignored --nocapture
# 1/2/4/8/... 동시 판 수를 단계적으로 바꿔 각3회 이상 반복한다.
python3 deploy/release_report.py --load crates/server/.tmp/load/실제결과1.json \
  --load crates/server/.tmp/load/실제결과2.json --load crates/server/.tmp/load/실제결과3.json
```

기본 CI 실행은1판8초의 기능 smoke이며 용량 검증이 아니다. 명시 부하 실행은 실패 관측도 JSON으로 보존하므로 테스트 exit0만으로 성능 성공을 판단하지 않는다. 각 결과의 errors/matched_players/ack_samples/applied_attacks와 p50/p95/p99를 읽는다. 측정은 로그인 교환·전세계 RTT·브라우저 LCP·서버 내부 입력 처리20ms의 대체가 아니다. 루프백 transport ACK에는 직렬화/IO/클라이언트·DB 지연이 포함되며 부하 클라이언트도 같은 장비의 CPU를 쓴다. native 실행과 실제 Compose의 CPU/메모리 제한은 자동으로 같아지지 않으므로, 운영 설정과 같은 자원 제한에서 측정하고 별도 외부 클라이언트/실제 배포의 교차 검수로 최종 승인한다.

report는 clean 동일 commit/설정/OS/CPU수의 release 측정 중60초 이상·표본100 이상·적용 공격10 이상·모든 요청 인원 매칭·오류0·전체/적용 ACK·적용 공격 p95≤200ms를 각3회 확인해야70% 이하 admission 후보를 낸다. 이를 실제 최대 용량 발견이나 운영 인증으로 표시하지 않는다. 실패 직전/회복 단계, 실제 장비 사양·메모리·CPU·모바일/해외 RTT·실제 LCP/입력 지연을 함께 검토하고 후보 이하로 설정한다. Compose는 `LIAR_ONLINE_MATCHES/CONNECTIONS/PROOF_WORKERS/MAILBOX/OUTGOING`, `LIAR_LOBBY_CAPACITY/WORKERS`, `LIAR_BOARD_CAPACITY/WORKERS`, `LIAR_BOT_WORKERS`를 전달한다. 설정을 변경하면 해당 limits에 맞는 측정·증거를 새로 기록한다. dirty/debug/짧은 표본·재사용한 run_id·다른 환경은 합산하지 않는다.

bundle 보고는 Vite manifest의 최초 정적 import 전체를 따라 파일별gzip 합계200KiB를 검사하며 lazy JS 전체와 WASM을 따로 기록한다. SW 선행 캐시와 최초 네트워크 전송량은 같은 값이 아니며 실제 브라우저 측정은 별도로 수행한다. 최신 `yarn build` 뒤 보고한다.
