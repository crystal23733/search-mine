# #227 권리 원장 암호화 보관·복구 검증

FR01/14/16 · NFR01/02/05/07/08 · TS20/21/28/29/31/34/35 → [ADR0104](../adr/0104-encrypted-recovery-checkpoint.md) · [운영 절차](../workflow/encrypted-rights-ledger.md).

## 구현과 인수 시나리오

native capture의 최소 권리 기록을 원래 expiry별 age 파일에 보관한다. 공개 manifest/인증 index는 source/capture/임의 파일명·원래 expiry·크기/ciphertext hash만 가지며 개인 UUID는 암호문 안에만 존재한다. 파일/디렉터리 fsync와 원자 current pointer, 전용 flock/regular/no-symlink 검증을 사용한다. 새 source/과거 capture 거절, 원래 기한 정리, 식별된 중단 잔여/이전 세대 정리를 구현한다. pointer commit 전 실패는 유효한 이전 current를 보존하고 commit 뒤 오류도 새 current가 가리키는 세대를 지우지 않는다.

verify/apply는 index와 현재 유효한 모든 개인 파일의 전체 age 인증·대응 검증 뒤에만 메모리 Snapshot을 Rust offline inspect-snapshot에 전달한다. 마지막 파일 손상/누락/다른 키는 target SQL 호출 전에 거절한다. apply는 같은 lock에서 검증된 자료만 기존 native apply-snapshot에 전달하고 성공도 quarantine이다. source/target URL은 secret 파일→자식 환경으로만 전달한다. 공개 recipient capture·비밀키 verify·key 없는 prune·target owner+비밀키 apply의 job/네트워크를 분리하며 기본 배포는 실행하지 않는다.

## Red/Green과 로컬 실행

설계/시나리오를 구현 전에 기록했다. native offline 검사 명령 미구현의 실제 assertion 실패를 확인한 뒤2개 CLI 검사를 통과했다. 초기에 oversized stdin helper가 조기 거절의 BrokenPipe를 오류로 처리했던 부분도 수정했으며 이를 제품 assertion 실패로 보고하지 않는다.

Python은 기존16개와 신규13개로29개 통과했다. 검토 중 index 인증 도중 원래 expiry를 넘으면 해당 파일이 다음 실행까지 남는 행동을 테스트로 재현(신규13개 중12PASS/1FAIL)하고, 인증 뒤 최종 prune과 선택 항목의 재검증으로13PASS를 확인했다. 이 밖에 source/Clock/원래 expiry·재캡처, index/마지막 파일 인증 전 DB 호출 차단, 누락/손상, 암호화 실패, pointer commit 뒤 fsync 오류, 중단 잔여/알 수 없는 파일·링크/동시 writer를 검증했다. 단위 crypto는 명시적 fake이며 실제 암호화 성공의 근거로 사용하지 않는다.

최종 로컬 Rust workspace485통과, fmt/clippy/WASM target/생성 타입 freshness 통과. 문서·Mermaid 검사는 PR 제출 전 실행 결과를 확인한다. 로컬에서 coverage를 추가 측정했다고 주장하지 않으며 정확한 source head의 CI 측정/전체 회귀를 PR 리뷰에서 확인한다.

실제 Docker 검증은 PG18.6/age1.3.2, image1096ff39e504bab646f757538ac7637ba8b5d8fabbf63789c56c45977d1b6a6b로 통과했다. 과거 dump를 빈 DB에 복원(gate DATA 제외/source 보존)하고, 이후 account/identity 원장을 public recipient만으로 캡처·offline 인증했다. 개인 UUID/닉네임 평문이 저장소에 없는지 검사했다. 마지막 파일을 누락시키거나 ciphertext를 손상한 뒤 공개 manifest/index hash까지 갱신해 단순 checksum을 우회해도 전체 age 인증이 실패하고 복구 DB의 account/identity2/2가 바뀌지 않았다. 다른 비밀키도 거절했다. 정상 적용은2회 모두 동일 결과·복구 account1/identity0/quarantine 유지였으며 제어 Clock의 원래 expiry에서 해당 개인 파일만 제거하고 기존 capture 시각을 유지했다. 전체 기한 뒤 세대 제거와 고유 fixture 정리를 확인했다. 실운영 RPO·RTO는 측정하지 않았다.

중간 Docker 실패는 daemon 중지로 build 전 중단 및 합성 target URL의 경로 변경이 username까지 바꾼 fixture 오류였다. Docker 시작/URI path 구성 수정 후 새 고유 fixture로 다시 확인했다. 처음 daemon 중지 때 cleanup을 확인할 수 없었던 project는 시작 후 컨테이너0을 확인했고, 이후 모든 fixture cleanup을 확인했다. 사용자 볼륨/타 프로젝트 이미지를 정리하지 않았다. 세부 실행 로그/원래 실패는 gitignored 로컬 기록에 보존한다.

## 남은 출구

이 단계는 신뢰하는 운영자 전용 저장소의 checkpoint이며 public recipient가 있는 자의 송신자 서명이나 모든 권리 변경의 최신 영구 보관 증명이 아니다. 동기 내구/최신 완전성·cutover/release·실제 별도 매체/복제본/스케줄/전원 중단·이전 실제 이미지/호스트 RPO·RTO는 #25에 남긴다. 사용자 파일 다운로드/API/화면과 기존 권리 삭제의 commit/실패 의미는 바꾸지 않았다. 실제 PWA 설치/OAuth/rollout은 #223이다.
