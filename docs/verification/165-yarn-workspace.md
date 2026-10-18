# #165 Yarn 워크스페이스 전환 검증

요구 NFR01/05/08·TS30, [ADR0078](../adr/0078-yarn-workspace.md), [이슈165](https://github.com/crystal23733/search-mine/issues/165).

## 변경과 설치

2026-10-18 사용자 지시에 따라 최신 안정 Yarn4.18.1/Corepack·루트 workspaces·node-modules 설치로 전환했다. 기존 두 선언 패치와 esbuild/unrs-resolver 빌드 허용을 옮기고 pnpm 전용 lock/workspace 파일을 제거했다. package scripts·PowerShell·Playwright webServer·CI·현재 개발 문서는 Yarn을 사용한다. 과거 실행 기록/보존 원문은 유지한다.

실제 pnpm lockfile/설치 manifest의 이행용 Classic seed를 Yarn으로 이행했다. npm 패키지719개 버전은 전부 유지·제거0, Yarn의 fsevents node-gyp 및 하위 빌드지원14개만 추가됐다. Babel7.29.7은 기존 자동 peer에서 명시 devDependency로 옮겼다. 최신8.0.7 설치는 실제 peer 지원 범위와 맞지 않았으며 단독 JSX 변환은 성공했음을 구분했다. 기존 타입 선언을 복원하고 prefresh의 누락된 preact 전달 metadata를 보충해 최종 peer 오류0을 확인했다.

반복 yarn install --immutable Exit0과 yarn.lock SHA256 불변을 확인했다. 기본 Git 허용 목록[]·npm age gate1440분과 enableScripts=false를 유지한다. unconfig의 boolean 선언과 sharp-ico의 불필요한 type import 제거가 실제 node_modules에 적용됐다.

## 실제 로컬 검증

- Corepack Yarn의 기존 pnpm 설정 거부 Exit1을 기준 상태로 확인했다.
- 기여 게이트의 pending-base 사례: 기존 코드는 yarn.lock/.yarnrc.yml/patch 변경을 통과시켜3개 Red, 변경 후 미승인 차단·승인 허용·문서 허용7개 Green. 첫 probe의 mock 구성 오류는 행동 Red에 합산하지 않는다.
- Rust fmt/clippy·단위431개 통과, PostgreSQL150개는 DB 없는 로컬 실행에서 ignored. 실제 DB 통과로 합산하지 않는다. WASM·생성 타입·native fixture·토큰 일치 확인.
- 웹49파일/366개, format/lint/typecheck·worker 진단8개·native trace 진단8개와 production build 통과.
- 첫 Chromium 전체는59통과·기존 #83과 같은 offline-mobile load10초 실패1·뒤의2개 미실행(1.1분). 최초 실패 trace/probe/native 자료를 .tmp/yarn165-local-browser-failure에 별도 보존했다.
- 같은 timeout/retry를 유지한 후속 PC/mobile offline 파일은8개/20.1초·재시도0 통과했다. 이 결과를 최초 전체 통과나 #83 해결로 합산하지 않는다.
- 문서·Mermaid34개와 diff 공백 검사를 수행했다. 최종 문서 수와 정확 source/원격 결과는 PR·로컬 일별 review에 기록한다.

첫 전체 검사는 C 드라이브 여유0으로 Rust linker/LLVM에서 실패했다. workspace 내 target 경로 확인 뒤 cargo clean -p liar-server로 재생성 가능한 서버 빌드 cache30.8GiB를 정리하고 다시 실행했다. 사용자 파일·.tmp 원자료는 삭제하지 않았다. 재실행은 format:check에서 명령 길이 변경에 따른 Playwright 서식1건으로 중단됐으며 prettier로 수정한 뒤 중단 지점 이후 실제 check 명령을 별도 실행했다. 단일 전체 check Exit0으로 표현하지 않는다.

## 리뷰와 원격 출구

root를 제외한 topological typecheck로 재귀 실행을 방지하고, 모든 workspace/binary 실행과 CI immutable 설치 경로를 검토했다. 기여 게이트에 Yarn 설정·lockfile·패치를 포함한다. 게임 규칙·TS 규칙 중복·계정/CMP/광고·서비스 워커·프로토콜/예산/timeout/retry는 바뀌지 않았다. Rust 두 진입점은 재생성 안내 문자열만 바뀌었다.

원격 필수6 checks·깨끗한 Linux 설치·실제 PostgreSQL/HTTPS/browser와 최신 head를 검토한 뒤 위임된 develop 병합·#165 종료를 확인한다. 실제 원격 실행 전 이를 통과했다고 기록하지 않는다. #83과 외부 OAuth42/CMP147·운영/사람 검수는 별도 미완료다. 로컬 USER_PREPARATION.local.md·review·이행용 .tmp·Yarn 캐시는 커밋하지 않는다.
