# ADR0109: 현재 로그인에서 명시 반영하는 Google TCF 선택

상태: 기존 FR13·현재 세션 동의 제어의 실제 adapter 구현 결정. #235, ADR0083/0084/0086/0088과 승인된 ADR0107을 따른다. 계정 접근 권한이나 게임 규칙을 바꾸지 않는다.

## 결정

Google 메시지는 브라우저 TCF 선택을 제공하며 사용자별 서버 원격 인증/서명 API를 제공한다고 가정하지 않는다. 서버는 현재 cookie·Origin/CSRF·계정/세션을 확인한 사용자가 **명시 반영한 선택**만 기록한다. 제출 TC string은 Google 서명 증명이 아니며 위조 불가능하다고 표현하지 않는다. 다른 계정·인증 권한·게임 결과를 이 문자열로 만들 수 없다.

refresh 요청에 닫힌 Google TCF evidence를 추가한다. 임의 목적 boolean/계정ID는 받지 않는다. 최대3072문자/4segment·선형 bit reader로 version2/core·Google CMP300·알려진 policy4/5·시각/해당서비스·범위/제약·publisher 목적을 검증한다. 큰 vendor range는 펼친 set을 할당하지 않는다. 프로토콜/정책 상수는 Rust 원천에서 TS로 생성한다. 실제 IAB CMP/GVL 자료에서 Google300/광고 vendor755·목적1/3/4를 확인했다.

서비스 자체 선택 분석은 publisher 목적1/8/10, 자체 광고 측정은 publisher 목적1/7과 Google 광고 동의를 모두 요구한다. 이 목적을 운영자가 Google 메시지의 자체 데이터 사용 항목과 실제 정책에 게시하지 않으면 수집을 허용하지 않는다. Google 광고 동의만으로 자체 분석을 허용하지 않는다. 해당 purpose 조합은 이 제품의 보수적인 선택 수집 정책이며 법적 준수 판정이 아니다.

서버 receipt는 최대1시간과 현재 세션 만료 중 짧은 기간이며 기존 revision CAS·동일 transaction at-write·철회/만료·현재 subject를 계속 요구한다. 원문 TC string은 검증 중 메모리에서만 처리하고 DB/로그/응답에 넣지 않는다. 기본/제공자 미설정은 그대로 unavailable이고 선택 이벤트는 닫힌 상태다.

브라우저가 이전 TC 선택을 읽었다는 이유만으로 서버의 철회를 자동 덮어쓰지 않는다. 공개 동의 관리에서 현재 로그인 선택을 명시 반영하고, 개인 화면의 재확인은 그 공개 문서로 이동한다. 브라우저 광고 허용과 서버의 자체 수집 권한을 별도 표시한다. account/visibility/tab/context 변경·불명·오류에서는 기존 클라이언트 guard와 현재 서버 동의의 교집합만 사용한다.

## 근거와 검증

[IAB 형식](https://github.com/InteractiveAdvertisingBureau/GDPR-Transparency-and-Consent-Framework/blob/master/TCFv2/IAB%20Tech%20Lab%20-%20Consent%20string%20and%20vendor%20list%20formats%20v2.md), [CMP 목록](https://cmp-list.consensu.org/v2/cmp-list.json), [GVL](https://vendor-list.consensu.org/v3/vendor-list.json), [Google 현재 API](https://developers.google.com/funding-choices/fc-api-docs)를2026-11-14 확인했다. 구현은 [WEB235 시나리오](../verification/235-public-launch-wiring.md)에 문자열 위조의 한계·큰 range·잘림/중복/시각/다른 세션·철회 경합·실DB를 포함한다. 실제 운영 정책/선택 화면/법적 근거 검토는147이다.

## 배포 구성과 문서

공개 운영자 구성 JSON은 법적 문서의 실제 운영자·연락처·시행일·개정판과8언어 개인정보/약관의 제목·본문을 담는다. 저장소는 가짜 법적 문구를 생성하지 않는다. 빌드는 이 자료를 길이·키·언어별로 검사하고 정적 HTML과 공개 JSON으로 게시한다. 미설정 문서는 준비 중/noindex이며 Google모드 빌드는 구성 없이 실패한다. Google의 frame/img/connect HTTPS origin 목록도 이 공개 구성으로 제한하고 실제 네트워크 검수는 #147로 유지한다.

Google 공개 문서만 요청별 nonce·strict-dynamic CSP와 strict-origin referrer를 사용한다. nonce가 들어간 HTML은304를 사용하지 않아 본문과 헤더가 다른 nonce를 갖지 않는다. 나머지 개인/게임/인증/오류 문서는 기존 CSP와 no-referrer를 유지한다. 광고 ID는 비밀값이 아니지만 실제 값 없이 ads.txt를 만들지 않는다. Docker 빌드는 공개 구성의 BuildKit secret mount를 사용하며 임의 운영자 경로를 이미지에 복사하지 않는다.

서비스 워커는 Google모드에서 문서 탐색을 먼저 네트워크로 요청한다(2초 내 실패/지연은 기존 정적 shell로 복귀). 개인정보 문서를 runtime cache에 저장하지 않는다. 공개 문서의 요청별 CSP nonce·referrer가 root shell의 이전 헤더로 대체되는 문제를 피하고, 오프라인일 때 기존 연습 경로를 보존한다. 기본 광고 비활성 빌드의 기존 cache-first 탐색은 유지한다.

## 실제 배포에서 확인한 PWA 주소 오류

합성 Google 브라우저의 실제 service-worker ready가90초 내 성립하지 않았다. 동일 검수 이미지에서 `GET /index.html?__WB_REVISION__=fixture`는 실제404였다. 문서 파일명은 Nginx의 공개 문서 URL이 아니므로 precache manifest의 root `index.html`은 `/`, locale HTML은 해당 깨끗한 문서 URL로 변환한다. root fallback handler도 동일한 `/` 키를 사용한다. revision 값·자산·정책 JSON은 유지하며 HTML alias를 공개하거나 기존 query/개인 route 차단을 풀지 않는다. 실제 Nginx의 worker 활성화·제어를 검증 출구에 추가한다.
