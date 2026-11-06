# #204 · 튜토리얼 소개

[ADR0094](../adr/0094-tutorial-guide-document.md) · FR08/11/12/13·NFR01/02/05/06/08 · TS13/18/19/20/23/26/30/31.

최신 develop e54ebd8 clean/origin 동기화·openPR0과 선행202CLOSED/PR203MERGED를 확인했다. 승인된 튜토리얼 소개를 상위23의 독립 하위204로 분리하고 설계/시나리오를 제품 코드 전에 기록했다. 실제 Red/Green/검증·리뷰/CI/병합은 실행 후 기록한다. 외부42/147·상위23의 실제 색인/정책/전문 번역·사람 검수와83은 이 구현으로 완료 판정하지 않는다.


## 실제 구현·검증

- 구현 전 App의 소개 heading/학습 상태 시나리오4개가 실패하고 실제 JS-off HTTP에서 /en/learn이404인 Red1을 확인했다. 원문 `.tmp/learn204-{unit,browser}-red.log` 및 browser trace를 `learn204-browser-red-artifacts`에 보존했다.
- TutorialGuidePage는 Ui의 locale/메시지만 사용하고 service/core/학습 쓰기를 호출하지 않는다. 기존 PlayBasics/Card와 공통 GUIDE_STEPS 메시지 식별자를 재사용했다. new/skipped/complete와 저장소 거부에서 mark0·training/practice/daily/board factory0을 확인하고 언어 변경 후 상태 보존을 검사했다.
- 실제 JS-off dev/preview8소개 및32공개문서, PC1440/mobile360·German 긴문구/가로넘침·키보드 Enter→기존 Rust/WASM 학습9칸, query/hash 제거와 새문서 진입·학습 시작 전 완료키null, JS 저장소 거부의 일본어 소개를 검사했다. SDK가 있는 문서→소개→실제 학습에서 원래1회 실행 외에 요청/실행0도 검증했다. 기존 SEO/자동광고 회귀를 포함한 최종20개가7.6초에 통과했다.
- 별도의 configured 빌드/환경값 없는 preview에서 실제 모바일 JS-off32문서의 canonical/9대체URL/OG·sitemap32, 소개canonical→실제tutorial noindex와 기존SPA head수명이 통과했다. 기본 dist에는 시험도메인/개인입력/test SDK마커가 없었다.
- 전체웹443 PASS(62파일), 라인3874/4235=91.47%. 소개 page3/3와 공통 steps1/1은 각100%다. 관련unit34 PASS. lint/typecheck/format/build·docs250/제품21/설계18/시나리오36·Mermaid34·diff 검사 통과. worker 소스/flags/retry/기한과 의존성/lockfile은 변경하지 않았다.
- 로컬 전체auth/실DB/LLVM·전체browser는 미실행이며 필수 원격 CI로 확인한다. 실제 사람의30초 이해·전문 번역/법적 정책·도메인/검색 색인과83의 원인 해결을 대신하지 않는다.

## 실제 코드 리뷰

소개는 App 첫방문 자동 학습 대상에 넣지 않고 독립 읽기 문서로 유지했다. 규칙/공통footer/정적 문서의 진입과 소개의 시작/복귀는 locale 고정 plain href여서 기존 NavigationPort의 query/hash 유지 계약이나 초대 복귀를 수정하지 않는다. 실제 tutorial의 최근 상태·단계/mark·무계정 학습·게임 DTO/규칙은 그대로다. SEO allowlist에만learn을 추가하고 광고의 홈/규칙 allowlist는 확대하지 않았다. 기존 escaping/head/정확origin/불변preview와 실제게임 검색 제외를 유지했다. 코드/diff에서 추가 수정 필요 사항은 발견하지 않았다. 원격 정확head/6CI·최종 리뷰/병합은 PR/당일review에 기록한다.
