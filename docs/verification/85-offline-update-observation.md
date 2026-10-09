# #85 두 탭 업데이트 진단 보존

FR10/NFR05/07 · TS18/20/26 · [ADR0018](../adr/0018-public-offline-cache-and-safe-update.md). 원인 미확정인 #83에서 진단 기반을 분리했다.

선행 #80/PR81은 develop c0fa4875에 병합·종료했다. #80의 새 온라인 복구4개는 CI에서 첫 실행에 통과했지만 기존 offline PC 두 탭 업데이트는 load10000ms 실패 뒤 재시도로 성공했다(전체84pass+1flaky). #74 첫 CI mobile도 같은 경로였다. 원인은 미확정이며 #83은 열어 둔다.

## 변경과 실제 관측

- CI browser-failures artifact를 always로 업로드해 재시도 성공의 최초 실패 trace도 보존한다. [공식 upload-artifact](https://github.com/actions/upload-artifact#uploading-hidden-files)의 hidden 기본 제외에 맞춰 선택한 test-results/진단 폴더에 `include-hidden-files: true`를 명시한다. 전체 `.tmp`나 secret 폴더는 선택하지 않는다.
- 시험 전용 observer가 클릭·전송·prepare/release·token별 응답·controllerchange/load와 CDP native worker 상태를 수집한다. runner 메모리와 ignored `.tmp/offline-update-probes` JSON/attachment를 사용해 재로드 전후를 연결한다. fixture teardown에서 초기 준비 실패도 수집하고, 멈춘 renderer의 마지막 상태 조회/observer 해제는 각각1초 진단 예산으로 제한한다.
- SW/업데이트 어댑터/activity·게임/인증/캐시 정책·제품/테스트 제한 시간·retry는 변경하지 않았다. worker debugger나 강제 활성화를 사용하지 않는다. CDP/page observer가 타이밍에 영향을 줄 가능성은 남는다.
- 기본 관측 PC/mobile 각8회16개가52.2초에 재시도 없이 통과했다. JSON 보존을 더한 PC/mobile 각32회64개는63pass/1fail(3.3분)이다.64개 파일 모두 두 탭 boot2를 기록했고 마지막 클릭→첫 controllerchange는17.9~67.7ms(중앙33.6ms)였다. 이번 반복에서 load 실패는 없었고 실패1개는 mobile repeat3의 이후 캐시 삭제·수동 reload 뒤 disabled 문구5000ms 검사다. trace를 보존했으며 이 원인도 미확정이다. 전체 성공이나 #83 해결로 보고하지 않는다.
- 기준 브랜치 비교 때 pnpm이 임시 경로로 의존성 junction을 바꾼 사실을 확인했다. 임시 worktree 제거 뒤 prettier/tsc/oxlint는 MODULE_NOT_FOUND로 미실행이었다. `pnpm install --frozen-lockfile`로 링크를 복구한 뒤 typecheck/lint는 통과했다. lockfile/라이브러리 버전은 변경하지 않았다.

최종 fixture 수집 경계의 format/typecheck/lint와 docs134/Mermaid34 실패0을 확인했다. `scripts/check.ps1`은 Rust fmt/Clippy/native/WASM/types/fixture/tokens·format/lint/typecheck 뒤 기존 queue-page 테스트5000ms에서1fail/175pass(176개,17.66초)로 실패했다. 전체 gate 성공으로 계산하지 않으며 #84에서 분리한다. 남은 fresh build/85 browser·CI/PR·병합은 대기 중이다. #84 단위 준비 진단과 #82 결과 조회는 이 진단 PR 병합 뒤 독립적으로 순차 진행한다. #83 제품 수정 완료나 외부 OAuth/하드웨어 검수를 주장하지 않는다.
