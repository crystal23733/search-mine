---
name: liar-issue-pr
description: Start and finish a Liar Sweeper issue with its milestone, develop-based branch, reviewable PR, and verified issue closure after an authorized merge.
---

# 이슈와 PR

루트 `docs/workflow/git-workflow.md`와 `docs/workflow/backlog.md`에서 선행 이슈·milestone·검증 기준을 읽는다. 원격 상태와 local dirty changes를 확인하고 사용자의 작업을 보존한다. 이슈가 없으면 맡긴 작업 범위 안에서 milestone/issue를 먼저 구성한다.

최신 develop에서 `<type>/<issue-number>-<kebab-slug>` 브랜치로 작업한다. Conventional Commit을 쓰고 승인된 범위만 commit/push한다. 검증 후 develop 대상 PR에 요구 ID·시나리오·실행 결과·설계 변경·`Closes #N`을 기록한다. GitHub 이슈 자동종료는 기본 브랜치에 병합될 때 작동하므로 develop 기본값을 확인한다.

PR 작성 뒤 링크와 핵심 검토 결과를 보고한다. 2026-10-07 사용자가 전체 작업의 순차 검토·병합을 에이전트에 위임했으므로 별도 사용자 승인을 반복 요청하지 않는다. diff/코드 리뷰·관련 테스트·checks/최신 head를 확인해 squash merge하고 이슈 상태를 조회한다. 자동 종료가 안 되면 해당 완료 이슈만 닫는다. 마일스톤의 모든 출구 조건 완료 후 닫고 다음 선행 충족 이슈로 진행한다.

이 스킬은 외부 연락, 유료 가입, 배포 계정 변경의 권한을 추가하지 않는다. 기존 PR/이슈가 있으면 중복 생성하지 않는다.
