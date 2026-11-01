# #178 · 내 정보 화면과 파일 다운로드 제거

[ADR0081](../adr/0081-my-info-and-export-removal.md)·[ADR0083](../adr/0083-my-info-client-and-export-removal.md). FR01/09/11/13/16 · NFR01/02/05/06/07/08 · TS17~22/24/26/29/34~36.

선행 #176/PR179·#177/PR180 병합·CLOSED와 develop6efec66 clean을 확인하고 `feature/178-my-info-pages`에서 작업했다. 기존 게임 결과/공식 데일리/공유 PNG·보존 원문/데이터는 유지한다.

## 변경과 실제 코드 리뷰

- `/me`·`/me/history`·`/me/account`·`/me/privacy`와 헤더/홈/저장 결과의 진입점을 연결했다. 요약은 서버 전체 기간·필터 결과를 표시하며, 20개 cursor 페이지와 본인 상세를 제공한다. 90일 보관 범위·중단/취소·0표본·null 통계/시간·확인 불가 상대·공식 데일리와 기기 연습을 구분한다.
- 기존 계정 관리/기기 설정/CMP port를 재사용했다. `/settings`는 비로그인 기기 설정으로 남는다. OAuth 복귀 경로는 `me`·`account` 고정 토큰을 추가했고 DB allowlist migration을 함께 반영했다.
- Me HTTP strict decode·64KiB·no-store·redirect 거부, 기존 인증 요청의 self scope를 사용한다. 기본 game scope의 닉네임 조건과 공유 요청 예산을 유지한다. 계정/세션 revision·필터/화면·오프라인 변화 시 개인 DTO를 비우고 취소·늦은 응답 무시를 확인했다. 동일 계정 변경 중 관리 폼은 유지하며 실제 계정/세션 상실 시 제거한다. 개인 화면은 noindex/noarchive이고 메모리 스냅샷을 영구 저장하지 않는다.
- 현재 동의 선택의 미확인/현재/만료를 표시하며 조회 시작 monotonic 시각으로 만료를 보수적으로 표시한다. 조회 결과를 수집 권한으로 사용하지 않는다. 실제 CMP 미설정은 계속 차단된다. Apple 철회 후 재시도/수동 안내는 계정 폼이 닫혀도 인증 작업의 메모리 피드백에 남는다.
- 파일 UI·문구·Blob/download sink·collector·port/DTO·생성 TS·서버 projection을 제거했다. `/api/v1/me/export`·`/api/v1/me/games/export`·`/api/v1/me/games/replay`는 설정/비설정 실행 모두404다. 기존 migration/저장 데이터·native replay 검증/보존·필수 이벤트·삭제는 유지한다. 이벤트/데일리 테스트는 실제 저장 자료와 새 조회 API로 소유권·공식 시간·보존 경계를 검증하도록 전환했다. 삭제된 Privacy coverage 그룹만 제거하고 Me/다른 도메인95%·서버/웹80% gate를 유지했다.
- 새 의존성·추가 개인정보·비밀값·보존 원문 변경은 없다. 과거 검증136의 삭제 파일 링크는 당시 커밋 permalink로 보존했다. admin·전역403/404·실제 OAuth/CMP 운영·#83 해결은 범위에 포함하지 않는다.

## Red → Green

시나리오/ADR을 먼저 보완했다. 실제 실행에서 self scope의 닉네임 없음1개, 새 고정 복귀 토큰1개, 구 API 제거1개, Me HTTP2개, controller4개, 화면5개의 행동 실패를 확인하고 구현 뒤 통과했다. 이후 동일 계정 변경 중 폼/조회 복구1개와 Apple 연결 해제 안내1개를 각각 Red로 재현하고 수정했다. 360px·최대16자 `W` 닉네임으로 전적 상세의 가로 넘침을 실제 브라우저에서 재현한 뒤 카드 내부 줄바꿈으로 해결했다. 컴파일·fixture·선택 테스트의 전체 coverage gate 오류는 행동 Red로 계산하지 않는다.

## 로컬 실제 실행

| 검사 | 결과 |
|---|---|
| Rust workspace | 440 passed, 실DB156 ignored (별도 실행) |
| PostgreSQL18 전체 통합 | 156 passed / 15.48초 |
| 웹 전체 단위 | 52파일·371 passed, line3443/3769=91.35% |
| 실제 HTTPS auth 전체 | 61 passed / 2.9분 (후속 Apple 안내·긴 닉네임 보완 전) |
| 최종 HTTPS 대상 | 4 passed / 18.5초; 8언어×4화면×1440/360px·긴 닉네임·키보드·다운로드0·구 API404, 닉네임 전/301초 경과 조회, 보류 응답 후 peer logout, Apple 안내 |
| 정적/생성 검사 | fmt·clippy·WASM check·생성 TS freshness·Yarn format/typecheck/lint/build 통과 |
| 문서 | 225 Markdown·제품21·설계18·시나리오36, Mermaid35/오류0 (최종 CI로 다시 확인) |

최종 모바일/PC 캡처는 `.tmp/my-info-layout/`에서 육안 검토했고 CI artifact에도 포함한다. 로컬 PowerShell `check.ps1` 자체·LLVM coverage·전체 브라우저125개 재실행은 하지 않았다. 실제 OAuth/CMP·운영 하드웨어나 사람 관찰 증거로 해석하지 않는다. 전체 CI와 정확 head 리뷰·병합/이슈 종료는 PR에 실제 결과를 기록한다. 로컬 상세 로그와 Red/trace는 Git에서 제외되는 `.tmp/me178-*`와 당일 `review/`에 보존한다.
