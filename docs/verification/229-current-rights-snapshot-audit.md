# #229 현재 권리 원장 비교 검증

FR01/14/16 · NFR01/02/05/07/08 · TS20/21/28/29/31/34/35 → [ADR0105](../adr/0105-current-rights-snapshot-audit.md) · [운영 절차](../workflow/encrypted-rights-ledger.md).

## 구현과 경계

native compare-snapshot은 capture와 공유하는 read-only/repeatable-read/2초 source 읽기 안에서 인증된 checkpoint와 현재 권리 기록의 집합을 비교한다. version/source/원래 capture·expiry·한도와 관측 시각을 검증하고, 관측 때 미만료인 kind/id/deleted_at/expires_at이 정확히 같아야 한다. 누락/추가/같은 개수의 다른 ID·kind·원래 시각은 실패한다. 순서와 이미 만료된 checkpoint 항목은 결과를 왜곡하지 않는다. 원래 capture 시각과 관측 시각은 별도 필드이며 개인 차이는 출력하지 않는다.

ledger audit는 같은 저장소 lock에서 전체 index/모든 현재 개인 파일 인증을 마친 뒤에만 source DB 비교를 호출한다. source owner+identity/DB 내부망의 명시적 profile을 추가했다. 기본 자동 실행/새 DB 쓰기/서비스 활성화/사용자 다운로드/API/UI 및 권리 commit 의미 변경은 없다. 원래 기한의 파일 정리는 기존대로 수행한다.

matched는 해당 source MVCC snapshot의 관측이며 검사 중/이후 commit된 쓰기까지 포함했다는 뜻이 아니다. source 손실/격리/출처 불명/한도·Clock·연결 오류는 일치를 추측하지 않는다. 영구 승인 파일이나 자동 release에 연결하지 않으며 writer 정지/동기 내구·최종 cutover/release와 실제 운영 출구는 #25에 남긴다.

## 실제 Red/Green·로컬 실행

설계와 시나리오를 구현 전에 작성했다. 실제 PG CLI에서 완전한 원장이 비교 성공해야 한다는 assertion의 Red1FAIL을 확인한 뒤 Green을 확인했다. model11개(신규4)와 관련PG15개(신규5)를 통과했다. Python audit port 미구현3errors→전체32PASS이며 이를3개 제품 assertion 실패로 보고하지 않는다.

신규 model 시나리오는 순서/두 시각 보존, 누락/추가/같은 개수의 ID·kind 교체/원래 시간·source 불일치, 원래 expiry 정확 경계/빈 집합, 미래 checkpoint/만료 자료/Clock 역행이다. 실제 PG는 CLI 고정 오류·개인 ID 비노출, gate 잠금 사이 commit한 새 항목이 기존 MVCC 관측에는 섞이지 않지만 다음 비교에는 불일치하는 점, live owner와 읽기 병행/상태 무변경,501개 감지/출처 누락/격리/Clock·2초 SQL 대기 거절을 확인했다. 원래 암호화 checkpoint는 덮어쓰지 않으며 마지막 인증 실패 때 source 비교도 호출하지 않는 Python 경계를 확인했다.

최종 로컬 Rust workspace489/fmt/clippy/WASM/type freshness PASS, 실제 PG226(42.20초, 전용 tmpfs2GiB·threads4) PASS, Python32 PASS. 문서277/시나리오 trace36/Mermaid34 PASS. 로컬 coverage를 추가 측정했다고 주장하지 않으며 정확한 source head CI 측정을 리뷰에서 확인한다.

실제 Docker PG18.6/age1.3.2/image560693dbbc28182474748406dc1a2e1de9b5d03d0034fd7cc5ace9218eb9d50e에서 old checkpoint verify2/matched2→새 unlink commit→기존 verify2 성공/audit 실패→재캡처3/matched3을 확인했다. source account/identity1/1과 복구 target2/4는 비교로 바뀌지 않았으며 격리 target을 source로 비교하면 실패했다. 전체 인증 뒤 복구 적용2회는 applied3/target account1·identity1/quarantine 유지였다. 마지막 파일 누락/손상·다른 키·원래 개별 expiry/전체 세대 정리·개인 평문 없음·key/망 분리·고유 fixture cleanup도 통과했다. 이를 운영 RPO·RTO로 간주하지 않는다.

첫 Docker 실행은 추가 제공자 fixture의 issuer가 기존 schema 제약과 달라 데이터 준비 중 실패했고, migration 계약의 고정 값으로 수정 후 새 고유 fixture에서 통과했다. 실제 제공자 요청은 보내지 않았고 사용자 볼륨/타 프로젝트 이미지를 정리하지 않았다. 원래 실패와 실행 로그는 gitignored 로컬 기록에 보존한다. 전체 CI/실제 산출물·정확한 head의 최종 리뷰는 PR에 기록한다.
