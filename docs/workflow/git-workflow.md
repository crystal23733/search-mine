# Git·GitHub 작업 규칙

## 브랜치와 커밋

기본·통합 브랜치는 **develop**이다. main은 선택적 공개 release로 보존한다. 기능마다 develop에서 분기하고 완료 PR도 develop을 대상으로 한다.

| 종류 | 예 |
|---|---|
| feature | feature/7-no-guess-generator |
| fix | fix/21-resume-idempotency |
| docs | docs/3-design-approval |
| chore | chore/2-project-foundation |
| refactor / test | refactor/25-bot-policy, test/30-load-scenarios |
| release | release/v0.1.0, develop→main 릴리스 검토 전용 |
| hotfix | hotfix/42-security-fix, main에서 분기 후 develop에도 반영 |

short kebab slug, 이슈 번호 필수. 오래된 `feature/front-atoms/develop` 관례는 새 작업에 사용하지 않는다. 기존 원격 브랜치는 임의 삭제하지 않는다.

커밋은 `type(scope): 한국어 변경 요약`, type=feat/fix/docs/chore/refactor/test/perf/ci/build/revert. 예: `docs(design): 거짓말 간파와 재접속 계약 정의`. 과거 `Test|` 이력은 변경하지 않는다. PR은 squash merge를 기본으로 하고 최종 제목도 Conventional Commit 형식을 따른다.

## 작업 생명주기

1. milestone 출구 결과와 작은 issue 인수 조건·PRD/TS ID·선행 이슈를 정의한다.
2. 이전 작업 PR이 병합·이슈 종료됐는지 확인한다. 최신 develop에서 분기한다.
3. 설계 승인 상태·요구 추적 확인. 승인 후 시나리오→Red→Green→Refactor.
4. 관련 검사와 diff·비밀 점검 후 commit/push, develop base PR을 연다.
5. PR에 `Closes #N`, milestone, 검증 결과, 설계 변경과 남은 가정을 적는다. 설계 승인처럼 별도 판단 이슈는 `Refs #N`으로만 연결한다.
6. 사용자 검토/수정 요청 반영. 리뷰 없이 자동 병합하지 않는다.
7. 명시 병합 승인과 checks 통과·최신 head 확인 후 squash merge한다. 완료 이슈 상태를 조회하고 자동종료 안 됐으면 수동 종료한다.
8. 모든 이슈와 출구가 충족된 milestone만 닫고 다음 작업을 시작한다.

GitHub 자동 이슈 종료는 PR이 기본 브랜치에 병합될 때 작동한다. develop이 기본이므로 `Closes #N`을 사용할 수 있지만 실제 상태를 반드시 확인한다. [공식 문서](https://docs.github.com/en/issues/tracking-your-work-with-issues/using-issues/linking-a-pull-request-to-an-issue)

## 1인 리뷰·브랜치 보호

자기 PR에는 공식 Approve 리뷰를 할 수 없어 required review count=0을 제안한다. 필수 PR·docs/contribution-gate checks·conversation resolution·force push/delete 금지와 **사용자의 PR 검토 및 수동/명시 승인 병합**을 결합한다. 사용자 리뷰를 자동 self-review로 대체하지 않는다. 추가 리뷰어가 생기면 count1로 바꾼다. GitHub 기능/플랜 제약으로 보호 API 적용이 불가하면 그 사실을 기록하고 workflow 검사는 계속 유지한다.

제품 경로 변경 PR은 base develop의 design-approval.json이 approved여야 CI가 통과한다. 승인 기록과 제품 구현을 같은 PR에 묶어 게이트를 우회하지 않는다. approved_by/approved_at/evidence/reviewed_commit을 승인 근거와 함께 기록한다. 사용자 채팅 또는 PR의 명시적 설계 승인 없이는 기록을 바꾸지 않는다.

main으로 release할 때는 release 브랜치와 별도 릴리스 이슈/PR을 사용한다. main을 기본 브랜치로 바꾸지 않으며 develop을 강제 reset하지 않는다.

## 초기 설정 권한과 보존

현재 사용자 요청은 초기 설정 commit/push/PR 제출을 포함한다. 기존 파일 삭제는 별도 사용자 확인을 받아 수행했다. .git과 docs/planning 원문을 보존한다. 원문은 과거 지시의 증거이고 workflow/기본값이 갱신됐다고 원문을 수정하지 않는다.
