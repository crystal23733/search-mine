# PWA 설치와 실기기 검수

[ADR0101](../adr/0101-web-workflows-and-installable-pwa.md) · [검증 #221](../verification/221-installable-pwa.md).

## 이용 방법

확정된 HTTPS 사이트의 `/{locale}/settings`에서 설치 안내를 연다. 브라우저가 설치 요청을 제공하면 앱 설치 버튼을 사용자 클릭에서 실행한다. 버튼이 없으면 브라우저의 앱 설치·홈 화면 추가·Dock 추가 메뉴를 확인한다. iPhone·iPad에서는 공유 메뉴의 홈 화면에 추가를 사용하며 구체 메뉴는 브라우저/OS마다 다를 수 있다. 취소/실패해도 같은 웹에서 계속 이용한다. 별도 파일이나 앱스토어 계정/결제를 요구하지 않는다.

설치한 아이콘으로 실행하면 지원 환경에서 앱 창으로 열린다. 설치와 캐시의 수명은 다르다. 캐시 준비 전 첫 오프라인 실행이나 브라우저가 삭제한 데이터를 보장하지 않는다. 공식 대전·본인 기록 조회·공식 순위에는 연결과 기존 서버 권한이 필요하다. 설치된 앱을 계정/동의/공식 시간 검증의 우회 수단으로 취급하지 않는다. 데이터 삭제/오프라인 캐시 관리는 기존 설정에서 처리한다.

## 배포와 개발자 확인

manifest의 id/scope/start_url은 `/`이며 계정·query·토큰을 넣지 않는다. manifest와 아이콘은 같은 origin의 공개 정적 자산이다. nginx는 `/manifest.webmanifest`를 `application/manifest+json`, `/icons/*.png`를 `image/png`로 제공하고 CSP는 same-origin manifest만 허용한다.32개 공개 locale HTML도 manifest를 연결한다. 인증/API/WS/광고 응답은 SW에 캐시하지 않는다.

기존 고정 SW URL·수동 등록·게임 중 업데이트 차단을 유지한다. 설치 코드는 자동 skipWaiting/reload·SDK/선택 수집을 추가하지 않는다. [ADR0102](../adr/0102-update-interaction-barrier.md)는 준비 ACK 전에8언어 modal로 링크 입력을 잠가 #83의 입력/Warm Up 경합을 보완한다. 로컬24+64회와 전체 CI는 [검증83](../verification/83-activation-recovery.md)/해당 PR에서 확인하며 실제 설치 기기의 업데이트 검수는 별도다.

아이콘 원본은 `apps/web/public/icons/source.svg`이고 기존 노란색 숫자4 브랜드를 사용한다. 현재 설치된 assets-generator로 아래 명령을 실행해 PNG/maskable/Apple 자산을 재생성한다. 아이콘은 앱 자산이며 사용자가 결과 파일을 다운로드하는 기능이 아니다.

```sh
yarn workspace @liar/web icons:generate
yarn build
```

## 실제 기기에서 남은 확인

CI의 합성 beforeinstallprompt/appinstalled는 포트·화면 계약만 검증하며 실제 OS 앱 설치가 아니다. 확정 HTTPS origin에서 다음을 실행하고 OS/브라우저 버전·날짜·결과를 남긴다. 실제 OAuth 설정은 #42, 외부 운영/접근성 종합 검수는 #23/#24와 함께 확인한다.

- PC Chrome/Edge, macOS Safari, Android Chrome, iPhone/iPad 지원 브라우저에서 실제 메뉴·설치 확인·아이콘/이름·독립 창 실행을 확인한다. 지원하지 않는 브라우저는 일반 웹 안내가 정상인지 확인한다.
- 설치 전후 선호 언어/기기 설정·일반 웹 이용과 실OAuth 로그인/로그아웃·제공자 복귀를 확인한다. OS별 저장소/쿠키 차이를 실제 확인하며 동일 세션 자동 이전을 추정하지 않는다.
- 캐시 준비 뒤 네트워크를 끊어 로컬 봇/데일리 연습과 공식 순위 연결 안내를 확인한다. 사이트 데이터 삭제 뒤 캐시 보존을 약속하지 않는지 확인한다.
- 실제 공개 배포 전환 시 진행 중인 게임·두 탭의 안전 업데이트와 설치 앱 재실행, 설치 해제 후 일반 웹 재방문을 확인한다. 실패 자료를 보존하고 검사 기한/재시도를 늘려 성공으로 덮지 않는다.

실제 OS 설치·확정 도메인/OAuth 복귀는 현재 미실행이며 이 문서는 완료 증거가 아니다. 현재의 실제 production/HTTPS/배포 검사와 원본 결과는 #221 PR의 검증 기록을 따른다.
