# 기여와 작업 시작

제품 구현 전 [설계 승인 상태](docs/workflow/design-approval.json), [PRD](docs/product/16-PRD.md), [AGENTS.md](AGENTS.md), [백로그](docs/workflow/backlog.md)를 읽는다. 브랜치·커밋·리뷰는 [Git workflow](docs/workflow/git-workflow.md)를 따른다.

이번 저장소에는 제품 실행 명령이 없다. 기존 Next/Nest 프로젝트는 초기화했고 Rust·web·Docker scaffold는 승인 후 M1 이슈로 만든다.

## 문서 검사

Python 3.11+로 저장소 루트에서 실행한다.

```powershell
python scripts/validate_docs.py
```

Mermaid 검사는 Node 22+와 별도 임시 의존성을 사용한다. 문서 검사 도구용이며 제품 package.json은 만들지 않는다.

```powershell
npm install --prefix .tmp/docs-tools --no-audit --no-fund mermaid@11.12.2 jsdom@26.1.0
node scripts/check_mermaid.mjs .tmp/docs-tools
```

`validate_docs.py`는 문서 목록·상대 링크·코드 블록·기획 결정/질문·설계 승인 상태를 검사한다. `check_mermaid.mjs`는 Mermaid 실제 parser를 사용한다. GitHub에서 PR의 렌더링도 확인한다.

## 제품 구현 이후

정확한 pinned 도구 버전과 `cargo test`, `clippy`, `pnpm`·DB·Playwright 명령은 M1 도구 체계 이슈에서 추가한다. 현재 미래 명령을 실행 가능한 것으로 안내하지 않는다. PR에는 변경된 행동, 왜 필요한지, Red/Green 결과, 필요한 검증과 미실행 환경을 적는다. 미니 PC 부하·복구는 실제 장비 성적을 첨부한다.
