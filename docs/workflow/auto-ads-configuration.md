# AdSense 자동 광고 설정

[ADR0091](../adr/0091-adsense-auto-ads.md) · #196/199 · 상위22/21와 외부147.

기본 광고 연결은 비활성이다. 실행 앱은 Google모드·공개 정책/CSP 구성·publisher ID가 모두 준비되어야 공식 CMP adapter를 연결한다. 게시자ID만 넣거나 script load가 성공했다고 실제 동의/광고 승인/노출/매출이 확인된 것은 아니다.

## 연결부

`apps/web/src/services/ads/types.ts`의 AutoAdsScriptPort를 DI한다. 현재 `ads/google.ts`는 pause를 먼저 설정한 단일 공식 태그를 CMP와 공유하며 광고 단위·수동 slot·adBreak 호출을 만들지 않는다. 다른 공급자로 바꿀 때 이 port 구현과 실제 운영 검수만 교체하고 게임 규칙/완료 카운터를 추가하지 않는다.

[출시 체크리스트](launch-checklist.md)의 `deploy/build_web.py`로 공개 정책/CSP 구성과 `VITE_CMP_PROVIDER=google`을 함께 설정하고, 실제 빌드 환경의 `VITE_ADSENSE_CLIENT_ID`에 본인 AdSense 계정의 게시자ID를 넣는다. 이는 공개 클라이언트 설정이며 API 비밀키가 아니다. 허용 형식은 ca-pub- 뒤16자리 숫자다. 코드/기본환경에 예시ID를 넣지 않는다. 없거나 형식이 다르면 네트워크 script0이다. 페이지HTML에 별도 광고script를 중복 삽입하지 않는다.

## 사용자가 할 실제 설정

1. 본인 계정/사이트 승인을 확인하고 실제 publisher 정보를 준비한다.
2. Google 개인정보 보호 및 메시지를 게시하고 구현된 ConsentPort에서 현재 문서/owner/context·목적/철회/만료를 검수한다. 테스트 grant를 실행 앱에 넣지 않는다.
3. [공식 Auto ads 설정](https://support.google.com/adsense/answer/9261307?hl=en)에서 승인된 사이트의 자동 광고를 켠다. 초기에는 인페이지 광고만 허용하고 overlay의 anchor/vignette/side rail과 ad intents를 끈다. [설정별 의미](https://support.google.com/adsense/answer/9305577?hl=en)를 확인한다.
4. 8locale별 게임/튜토리얼/데일리/queue/friends·결과/내정보/인증/설정/오류를 page exclusions로 제외한다. [제외 URL](https://support.google.com/adsense/answer/9262311?hl=en)의 query/hash 제한을 따른다. 코드는 query와 알 수 없는 hash를 제외하며 공개 문서의 고정 접근성 #main 앵커는 허용한다. 홈/규칙만 초기 대상이며 추가 페이지는 별도 검수한다.
5. 실제 script/frame/img/connect 관측으로 최소 CSP allowlist를 검증한다. wildcard/전역 unsafe-inline을 쓰지 않는다. 운영 ads.txt에는 실제 publisher 레코드만 넣는다.
6. 실제 PC/mobile에서 동의/거부/철회·8locale/첫방문·광고 차단·느린응답·게임 진입/뒤로가기·오프라인·업데이트를 확인한다. page exclusions/설정 반영 지연도 고려하며 광고를 누르도록 테스트하지 않는다.

## 코드가 보장하는 경계와 한계

공개 홈/규칙/동의 관리 문서에서 온라인/visible/초기 인증 확인·게임 비활성 상태에 paused 태그를 문서당1회 준비한다. 현재 광고 동의 후 홈/규칙에서만 요청을 재개한다. 동의 전에도 script와 기존 Google 쿠키 읽기가 가능하다. 메모리 전용 학습/설정 환경에서는 광고를 생략한다. 보호 문서의 게임 DOM/URL 정보가 남아 있으면 SDK를 활성화하기 전에 새 공개 문서를 연다. 태그 준비가 시작된 문서는 게임/개인 화면을 SPA로 표시하기 전에 새 문서로 이동한다. 로딩 실패/timeout도 실행 가능성이 있었던 문서로 다룬다.

철회/owner 변경/오프라인/예상치 않은 게임 진입·복원은 문서 종료 경계에서 처리한다. script 태그 제거만으로 이미 실행된 SDK를 종료했다고 주장하지 않는다. 실제 Google/TCF 동작과 최종 노출은 운영147에서 확인한다. script load를 impression으로 집계하거나 승인 전 매출을 추정하지 않는다.

미설정·CMP 실패·차단은 게임/API 오류로 전파하지 않는다. 실제 설정 완료 전 상위22/21/147은 열어두고 독립 구현/테스트 성공과 운영 성공을 구분한다.

Google모드의 공개 문서만 nonce/strict-dynamic CSP와 strict-origin referrer를 사용한다. 실제 SDK의 허용 origin 관측과8언어 정책 확인은 #147에 남는다. 정책 자료가 없으면 미게시로 표시하고 Google모드 빌드를 실패시킨다. `/consent` 자체에는 광고를 표시하지 않는다.
