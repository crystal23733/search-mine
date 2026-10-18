# ADR0078: Yarn 워크스페이스와 재현 가능한 설치

상태: 승인된 사용자 도구 교체 지시의 구현 결정, 2026-10-18.

## 근거와 범위

사용자가 `yarn으로 교체`를 지시했다. #165는 선행 #4·#163이 병합·종료된 develop에서 진행하는 독립 도구 변경이다. NFR01/05/08·TS30의 설치/빌드/검증 재현성을 유지한다. 원문과 과거 실행 기록은 보존하고 현재 운영 명령만 바꾼다.

## 결정

- npm registry에서 확인한 최신 안정 Yarn 4.18.1을 packageManager에 기록하고 Corepack으로 실행 버전을 선택한다. 라이브러리 추가 명령에는 버전을 지정하지 않는다.
- workspaces를 루트 package.json의 apps/*·packages/*로 옮긴다. nodeLinker는 node-modules로 명시하여 현재 Vite/Pixi/WASM·도구의 파일 접근 모델을 유지한다.
- yarn.lock을 커밋하고 로컬 재설치·CI는 yarn install --immutable로 검사한다. 기존 라이브러리 manifest 범위는 유지하며 전환 중 해결된 버전 차이는 lockfile 비교와 실제 실행으로 확인한다.
- sharp-ico와 unconfig의 기존 선언 패치는 Yarn patch: resolution으로 재사용한다. runtime 코드나 skipLibCheck로 바꾸지 않는다.
- enableScripts=false와 루트 dependenciesMeta의 built=true로 esbuild·unrs-resolver만 의존성 빌드를 허용한다. 기존 pnpm 허용 목록과 같은 경계다.
- 루트 script·Playwright webServer·PowerShell 검사·GitHub workflows를 Yarn으로 통일하고, .yarnrc.yml·yarn.lock·패치를 기여 게이트의 제품 설정 경계에 포함한다. 캐시와 install-state는 추적하지 않는다.

## 검증과 한계

기존 pnpm lockfile의 실제 설치 manifest/해결 버전으로 일회성 Classic lockfile을 만들고 Yarn 공식 이행으로 yarn.lock을 생성했다. 원래719개 패키지 버전이 모두 유지되고, Yarn의 fsevents 빌드 지원 node-gyp 및 하위14개만 추가됐음을 비교했다. 이행용 스크립트와 이전 lockfile 사본은 .tmp에만 둔다.

pnpm 자동 peer 설치가 보충하던 Babel을 웹의 명시 devDependency로 옮긴다. 버전 인자 없는 add는 최신8.0.7을 설치했지만 최신 preset-vite2.10.6은7.x, hook-names1.0.2는^7.12.10을 요구함을 실제 registry/설치의 YN0060으로 확인했다. 단독 JSX 변환은8에서도 성공했으므로 runtime 실패로 기록하지 않는다. 지원 계약과 기존 버전 보존을 위해 remove 후 add -D --cached로 원래7.29.7을 선언하고 Babel7 타입을 복원했다. 임의 exact downgrade/resolution을 추가하지 않았다. preset-vite가 하위 prefresh에 전달하지 않은 preact peer는 packageExtensions로 실제 요구 범위를 전달하며 범위를 완화하지 않는다.

Classic 이행이 자동 추가한 approvedGitRepositories=[**]·npmMinimalAgeGate=0은 제거했다. Yarn 기본값(빈 Git 허용 목록·1440분 age gate)을 유지하며 기존 allowlist 밖의 install script를 허용하지 않는다.

현재 pnpm packageManager 때문에 Corepack Yarn 실행이 거부되는 기준 상태를 확인했다. 변경 후 immutable 설치·workspace 연결·두 선언 패치·단위/typecheck/lint/build·실제 Chromium/HTTPS/PG와 기존 필수 CI를 검증한다. 반복 설치가 yarn.lock을 바꾸지 않는지 확인한다.

게임 규칙·공개 프로토콜·성능 예산·timeout/retry는 유지한다. 기존 #83의 간헐 두 탭 업데이트 실패는 별도 자료를 보존하며 Yarn 전환 성공으로 해결됐다고 보고하지 않는다. 실제 OAuth42·CMP147·운영/사람 검수도 별도 미완료다.

공식 근거: [Yarn 설정](https://yarnpkg.com/configuration/yarnrc), [workspace 실행](https://yarnpkg.com/cli/workspaces/foreach), [선언 패치](https://yarnpkg.com/features/patching), [설치](https://yarnpkg.com/cli/install).
