# ADR0101: 파일 없이 사용하는 웹과 선택적 PWA 설치

- 상태: 채택 — 2026-11-10 사용자의 명시적 개발 지시
- M2 #219 설계 → 후속 제품 이슈 · 선행 #217/PR218 병합·종료
- FR01/09/10/11/16 · NFR01/02/03/05/06/07/08 · TS17/18/19/20/24/26/30/36
- 근거: [승인 기록](../workflow/approval-record.md)

## 웹 안에서 처리

내 정보·보관 전적·계정/기기 설정·동의·공식 순위는 웹 화면에서 조회·처리한다. 개인 데이터 다운로드는 #178에서 제거했다. 남아 있는 데일리 PNG 저장 버튼·canvas/Blob/임시URL 다운로드 adapter·번역·시험을 제거한다. 결과는 기존 웹 카드/텍스트 모자이크로 표시하고 사용자의 복사·텍스트 native share만 유지한다. 공유 취소/미지원에도 결과를 계속 읽고 복사할 수 있다. 랭킹은 기존 `/daily/rankings`의 서버 검증 시간/검증 완료 두 목록과 새로고침을 유지하며 로컬 기록·파일을 순위 권위로 쓰지 않는다. 과거 PNG/export 실행 문서는 이력이며 현행 요구가 아니다.

백업·삭제 원장은 운영자 내부 복구 작업이다. 사용자 웹 화면의 기능이나 파일을 받아야 하는 이용 절차로 노출하지 않는다. 이번 변경은 그 운영 데이터의 보존·접근 권한을 확대하지 않는다.

## 선택적 설치

기존 injectManifest/SW와 캐시를 재사용하고 Web App Manifest를 제공한다. 안정된 id/scope/start_url은 `/`, display는 standalone, 이름은 Liar Sweeper다. query·계정·언어·토큰을 설치 ID/시작 URL에 넣지 않는다. 시작 페이지는 기존 선호/브라우저 언어 선택을 사용한다. 프로젝트 원본 아이콘으로192/512 PNG·maskable512·Apple touch180을 제공하고 실제 정적 HTML·Vite preview·nginx에서 manifest/icon URL을 HTML404로 바꾸지 않는다. CSP는 same-origin manifest만 허용한다.

설정에8언어 설치 카드를 둔다. early composition root의 InstallPort가 beforeinstallprompt를 보존하며 버튼 클릭의 동기 호출에서만 prompt()를 실행한다. 이벤트 한 번당 한 번만 사용하고 중복 클릭·취소·거절·예외는 사용자 기능/기록을 막지 않는다. 자동 팝업·계정/결제·파일 다운로드·별도 SDK는 추가하지 않는다. prompt 미지원/이벤트 없음에는 브라우저의 설치 메뉴 및 iPhone/iPad의 공유→홈 화면 추가 안내를 표시한다. 지원 여부를 UA로 단정하지 않고 버튼은 실제 보존한 prompt가 있을 때만 활성화한다.

standalone/display-mode 및 Apple standalone은 현재 앱 창 실행의 증거로만 표시한다. 일반 탭의 이벤트 부재를 설치 완료로 추정하지 않고 accepted 결과만으로도 설치 완료를 단정하지 않는다. appinstalled를 관측한 경우와 앱 창 실행을 구분한다. 설치 상태·prompt 객체는 메모리에만 보관하며 외부 수집을 추가하지 않는다. 구독·media/event listener를 해제하고 미완료 promise의 늦은 결과를 무시한다. 설치 거절/지원 없음에도 일반 웹 이용을 유지한다.

## 기존 불변 조건과 검증

공식 대전/기록/랭킹은 연결과 서버 검증이 필요하다. 캐시가 준비된 로컬 봇·데일리 연습만 오프라인으로 제공하며 PWA 설치를 공식 offline 기록 승인으로 취급하지 않는다. 인증/API/WS/광고 비캐시와 게임 중 자동 skipWaiting/재로드 금지를 유지한다. 캐시·설치 실패/저장소 삭제는 일반 웹을 막지 않으며 설치 완료를 영구 캐시 보장의 증거로 삼지 않는다. #83 두 탭 간헐 업데이트는 별도 OPEN이다.

시나리오를 먼저 작성한 뒤 install port의 이벤트 전/후·one-shot·동기 user gesture·중복/취소/예외/폐기·실행 모드와 UI의8언어/접근성/설치 후 상태를 Red→Green으로 검증한다. 실제 production HTTPS에서 manifest/아이콘/MIME/CSP·공개8언어문서·오프라인 시작/기존 안전 업데이트·내정보/공식순위와 다운로드0을 검사한다. 합성 beforeinstallprompt/appinstalled는 UI 수명 계약만 증명하며 OS 설치 완료를 증명하지 않는다. 실제 Android/iOS/desktop 설치·홈화면 실행·OAuth 복귀/업데이트는 확정 HTTPS origin과 실기기의 후속 검수로 남긴다.

[MDN 설치 요건](https://developer.mozilla.org/en-US/docs/Web/Progressive_web_apps/Guides/Making_PWAs_installable), [사용자 설치 요청](https://developer.mozilla.org/en-US/docs/Web/Progressive_web_apps/How_to/Trigger_install_prompt), [실행 모드](https://developer.mozilla.org/en-US/docs/Web/Progressive_web_apps/How_to/Create_a_standalone_app), [Vite PWA 최소 요건](https://vite-pwa-org.netlify.app/guide/pwa-minimal-requirements.html)을2026-11-10 확인했다. 브라우저 메뉴/설치 지원은 차이가 있으므로 단일 자동 prompt를 모든 브라우저에 약속하지 않는다.
