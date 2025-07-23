# Mines Battle

지뢰찾기 PvP 웹게임 – Next.js, NestJS, PostgreSQL, OAuth, i18n, 실시간 매칭, 랭킹, 아이템 시스템

---

## 프로젝트 구조

```
mines-battle/
├── frontend/      # Next.js (TypeScript) 클라이언트
├── backend/       # NestJS (TypeScript) API 서버
├── libs/          # (공통 타입, 유틸 – 선택)
├── infra/         # (Docker, Nginx 등 – 선택)
├── README.md
└── ...
```

---

## 주요 스택

* 프론트: Next.js, TailwindCSS, Framer Motion, react-i18next, next-auth
* 백엔드: NestJS, Prisma, nestjs-i18n, Passport, JWT, WebSocket, Socket.io
* DB: PostgreSQL, Redis
* 인증: Google OAuth (Auth0, Cognito 가능)
* 테스트: Jest, Testing Library
* 배포: Docker, Nginx
* 광고: Google Adsense

---

## 초기 세팅

### 1. Yarn 설치

```bash
npm i -g yarn
```

### 2. 레포지토리 설치

```bash
git clone <repo_url>
cd mines-battle
yarn install
```

### 3. 환경 변수 설정

* `.env.example` 참고 후 각 서비스별 `.env` 작성

---

## 공통 스크립트

| 명령어                 | 설명              |
| ------------------- | --------------- |
| `yarn dev`          | 프론트/백 동시 개발모드   |
| `yarn dev:frontend` | 프론트 개발모드 (3000) |
| `yarn dev:backend`  | 백엔드 개발모드 (4000) |
| `yarn lint`         | 전체 코드 스타일 검사    |
| `yarn test`         | 전체 테스트 실행       |

---

## Git 컨벤션

### 브랜치 규칙

* `main`: 배포용
* `dev`: 통합 개발
* `feature/기능명`
* `fix/버그명`
* `docs/문서명`
* `hotfix/긴급패치명`

### 커밋 메시지 규칙

* 타입: `feat`, `fix`, `chore`, `docs`, `refactor`, `test`, `style`, `perf`
* 예시:

  * `feat: 랭킹 시스템 추가`
  * `fix: 로그인 실패 예외처리`
  * `chore: eslint 설정 업데이트`
  * `docs: ERD 다이어그램 추가`
* 커밋 메시지는 한글/영문 자유

### PR 규칙

* 제목: `[타입] 기능설명`
* 본문: 구현내용/테스트/리뷰포인트 등 상세 기재
* 리뷰어 1명 이상 지정

---

## 코드 컨벤션

* Prettier/ESLint 적용 (자동 포맷팅)
* 타입스크립트 필수
* 폴더 구조는 기능별/도메인별로 분리