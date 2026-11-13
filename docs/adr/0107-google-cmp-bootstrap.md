# ADR0107: Google 개인정보 보호 및 메시지의 공식 bootstrap

상태: 승인. #233, FR12/13, 상위 #21/#22/#147. 사용자가2026-11-13 Google ‘개인정보 보호 및 메시지’를 선택했다. 같은 날 사용자가 동의 전 태그·기존 쿠키 경계를 설명한 질문에 “Google 공식 방식으로 진행”을 명시 선택했다. 설계 기록 병합 전 제품에는 적용하지 않는다.

## 확인한 문제

기존 [ADR0091](0091-adsense-auto-ads.md)은 현재 광고 동의가 유효한 뒤에만 AdSense script를 로드한다. Google의 공식 EU 메시지 설치 절차는 사이트에 AdSense 태그가 있어야 하므로 그대로 연결하면 최초 메시지와 동의가 서로를 기다린다. 확인하지 않은 독립 Funding Choices URL을 추정해서 넣지 않는다.

Google은 AdSense 태그 로드 전에 `pauseAdRequests=1`을 설정하고, 동의 후에만0으로 바꾸는 방식을 문서화했다. 이때 광고 요청은 중단되지만 관련 script는 로드되고 기존 Google 도메인의 쿠키가 읽힐 수 있다. Google 문서는 이 데이터가 광고 제공/측정에 쓰이지 않고 새 쿠키는 설정되지 않는다고 설명한다. 이를 “Google에 아무 연결도 없다”로 안내하지 않는다.

## 검토할 선택

1. Google 공식 방식: 공개 홈/규칙·별도 공개 동의 관리 문서에서만 태그를 광고 요청 중단 상태로 먼저 로드한다. 광고 허용 선택을 확인한 뒤 홈/규칙에서만 광고 요청을 시작한다. 거부/실패/불명/적용 지역 미확인에서는 요청0을 유지한다. Google 메시지 게시·세계 지역·실제 쿠키/네트워크 검수는 운영자가 한다.
2. 기존 차단 유지: 동의 전 Google 태그를 로드하지 않는다. 현재 Google 메시지는 최초 방문에서 자동 동작하지 않으므로 광고도 비활성으로 유지한다. 별도의 공식 지원 경로나 동의 관리 선택이 확인된 뒤 재검토한다.

## 승인된 공식 방식의 구현 경계

처음부터 중단 설정을 적용한 단일 shared client만 태그를 소유한다. CMP와 광고 활성화가 별도 script를 중복 삽입하지 않는다. 태그가 한 번 시작된 문서는 광고 허용 여부와 무관하게 게임/개인/인증 화면으로 SPA 이동하기 전에 새 문서로 바꾼다. 동의 관리는 공개 문서로 이동해 처리하고 계정·전적·게임 문서에는 Google 태그를 넣지 않는다. 광고 거부 때문에 게임을 기다리게 하지 않는다.

현재 TCF `addEventListener`와 `googlefc.callbackQueue`/`showRevocationMessage`를 사용한다. 폐기된 `getConsentStatus`/`getTCData`로 허용하지 않는다. 불명·오류·동의창 표시 중·철회·context 변경은 닫힌 상태로 처리한다. 브라우저가 보고한 선택을 Google의 서명 또는 서버 원격 인증으로 오인하지 않는다. 선택 이벤트의 서버 저장은 같은 계정/세션의 별도 현재 목적 동의와 CAS를 계속 요구한다. 실제 목적·정책 설정 없는 분석/광고 측정을 켜지 않는다.

CSP는 문서별로 분리하고 임의 URL·전역 wildcard/스크립트 unsafe-inline을 허용하지 않는다. nonce 호환성과 실제 계정에서 관측한 script/frame/connect/img를 검수한다. Google 메시지는 cross-origin referrer를 요구하므로 태그 허용 공개 문서만 origin 수준 referrer를 사용하고 개인/인증 문서의 no-referrer를 유지한다. 오류/실패가 사용자 개인정보를 포함한 외부 URL·로그를 만들지 않도록 검증한다.

## 공식 근거 (2026-11-13 확인)

- [메시지 생성·AdSense 태그/referrer 요구](https://support.google.com/adsense/answer/10960768?hl=en)
- [광고 요청 중단·관련 script/기존 쿠키의 경계](https://support.google.com/adsense/answer/7670312?hl=en)
- [Google 메시지 API·현재 TCF 구독과 다시 열기](https://developers.google.com/funding-choices/fc-api-docs)

사용자 선택을 승인 기록에 남기고 설계 PR이 develop에 병합된 뒤 해당 제품 연결을 시작한다. 실제 운영자/법적 고지/계정 승인과 실제 SDK 네트워크 관측은 #147로 유지한다.
