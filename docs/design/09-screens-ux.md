# 09. 화면·조작·접근성

#80의 [ADR0038](../adr/0038-bounded-online-reconnect.md)은 본인 복구 중 읽기 전용 보드와 보드 앞8언어 안내를 유지한다. 표시하는30초는 로컬 시도 예산이고 상대 유예와 구분한다. 새 권위 snapshot 뒤에만 입력을 다시 켜며 자체 승패/저장 성공을 만들지 않는다.

#76의 상대 재연결 상태는 [ADR0036](../adr/0036-opponent-reconnect-grace.md)과 디자인15를 따른다. 보드 앞의 공통 ReconnectNotice 카드에서 양수 유예의 안내·남은 초·진행 막대를8언어로 제공한다.0은 “서버 결과를 기다리는 중”이며 클라이언트가 승패를 선언하지 않는다. 상대 복귀/서버 종료/본인 연결 종료 때 안내가 사라지고 본인 연결이 유지된 동안 계속 플레이할 수 있다.

#70의 /queue 빠른 대기·3난이도 봇·준비·실제 서버 보드·결과/기록과 로그인/오류 출구는 [ADR0033](../adr/0033-online-quick-match-ui.md)을 따른다. 서버 상태와 표시용 남은 시간을 분리하고 번역 변경에 진행 상태를 유지한다. 친구 방 UI는 다음 #17 하위 작업이다.

#72의 /friends는 [ADR0034](../adr/0034-friends-room-ui.md)의 디자인06·8문자 코드·명시 참가/OAuth 복귀·공개2seat/ready·나가기·서버 만료를 연결하고 실제 온라인 보드/결과를 재사용한다. 방 코드가 세션을 대신하지 않으며 공유 링크에 다른 정보는 넣지 않는다.

> 대응: FR06~13, NFR06 · [여정](../product/04-journey.md)

```mermaid
flowchart TD
  Entry[언어 URL과 초대 링크] --> Consent[필수 저장소 고지와 CMP 선택]
  Consent --> Auth[OAuth 최소 로그인]
  Consent --> Local[무계정 로컬 연습]
  Auth --> Nickname[게임용 닉네임]
  Nickname --> Tutorial[첫 접속 튜토리얼]
  Local --> Tutorial
  Tutorial --> Home[홈]
  Home --> Quick[빠른 대전 대기]
  Home --> Friend[친구 방]
  Home --> Daily[데일리]
  Home --> Settings[설정과 접근성]
  Quick --> Match[대전]
  Friend --> Match
  Match --> Result[결과]
  Daily --> Result
  Result --> Home
  Result --> Friend
  Result --> Share[데일리 카드 공유]
  Home --> Offline[서버 장애 로컬 연습]
```

CMP는 광고 선택 흐름이며 미동의도 OAuth/로컬 연습/튜토리얼로 진행한다. 초대 링크의 만료/입장 상태는 튜토리얼 뒤 확인하며 방 코드 외 신원정보를 URL에 넣지 않는다.

```text
홈 (좁은 화면)
┌──────────────────────────┐
│ Liar Sweeper  [언어] [설정]│
│ [닉네임]                 │
│ [빠른 대전] [친구 대전]   │
│ [오늘의 퍼즐] [규칙 배우기]│
│ 서버 상태 / 로컬 연습     │
│ 메뉴 배너 고정 영역       │
│ 정책 · 약관 · 동의 설정   │
└──────────────────────────┘

대전
┌──────────────────────────┐
│ 상대: 봇/친구  진행률  시간│
│ 내 진행률  게이지 [공격]  │
│ 보드: 확대·이동 가능한 격자│
│ 숫자·깃발·기절 상태       │
│ [열기] [깃발] [지목] 모드 │
│ 연결 상태 / 도움          │
└──────────────────────────┘

결과
승패/무승부/장애취소 · 이유 · 진행률
지목/반사/실수 · [재대결] [홈]
데일리: UTC 날짜 · 검증 상태 · [공유]
광고는 허용 조건일 때만 결과 뒤 표시
```

## 입력

PC: 클릭 open, 우클릭 flag, 열린 숫자의 메뉴에서 accuse, A로 지목 모드, Space/Enter 실행, 화살표 셀 이동. 모바일: tap 현재 모드, 450ms long press로 메뉴, 이동/pinch를 클릭과 구분하고 long press 후 tap 중복 발생 방지. 공격은 별도 버튼이고 상대 셀 선택 UI는 없다. 설정에서 동작 도움을 즉시 볼 수 있다.

## 반응형·접근성

<768px는 단일 보드, ≥768px도 상대 보드는 진실 노출 방지를 위해 진행 요약만. 16×16 전체를 360px에 억지로 넣으면 터치 영역이 부족하므로 기본 확대+pan, 44px 이상 액션 버튼, keyboard zoom과 현재 좌표 안내를 제공한다. 가로/세로 회전은 상태를 유지한다.

접근 가능한 DOM grid에 cell label(좌표·공개 숫자·flag·상태), roving tabindex와 aria-live 결과/기절/연결을 둔다. 공개 숫자만 읽으며 숨은 true value·lie 여부를 aria-label에 넣지 않는다. 키보드만으로 대전·지목·설정·동의 가능. reduced-motion, 충분한 대비, 색+아이콘, 광고와 조작의 간격, 초점 복구를 E2E와 수동 검수한다.

오류 상태는 만료 방·full·unsupported version·reconnect·offline cache missing·daily rejected·ads unavailable을 안정된 번역키로 제공한다. 서버 장애는 승패로 표시하지 않고 “대전 취소”라고 표시한다.

## 제공된 디자인 적용

[18 화면별 기준](18-design-layout.md)에 design_layout 원본 18개와 상태·반응형·토큰·차이를 연결했다. 로그인 기본 영역에 Google·Apple·카카오·네이버를 제공한다. 이미지의 Discord 기본 버튼은 확장 영역 후보로 이동한다. 로그인 없이 즉시 가능한 기능은 로컬 연습이며 온라인 대전은 OAuth가 필요하다. 닉네임은 제공자 실명 대신 직접 정한다.
