# #235 공개 웹 연결과 출시 회귀

요구: FR11/12/13/16·TS18/19/20/21/23/24/25/26/29/30/31/35/36. 선행 #233/PR234 승인 기록 병합·CLOSED 확인. [ADR0107](../adr/0107-google-cmp-bootstrap.md)과 기존 동의·저장소·문서 격리를 따른다.

## 인수 시나리오

| ID | 관측 결과 |
|---|---|
| WEB235-01 | Google모드 미설정은 Google 연결0. 공개 문서에서 pause를 먼저 설정한 단일 태그만 준비하며 현재 광고 동의 전 요청을 재개하지 않는다. |
| WEB235-02 | CMP의 거부/불명/오류/창 표시/만료/다른 context는 닫힌 상태다. 다시 열기와 현재 세션의 명시적 선택 반영·CAS/철회를 지원하며 임의 client boolean은 권한이 아니다. |
| WEB235-03 | 태그를 준비한 순간부터 보호 문서로 SPA 화면을 게시하지 않고 새 문서로 이동한다. 게임·개인·인증에는 태그0이고 동의 실패가 게임을 막지 않는다. |
| WEB235-04 | 실제 운영자가 확정한8언어 개인정보/약관 자료만 게시하고 HTML은 escape한다. 미설정은 준비 중으로 표시하며 가짜 운영자·정책·ads.txt·승인을 만들지 않는다. |
| WEB235-05 | 공개 Google 문서만 nonce/referrer/CSP 경계를 사용하고 실제 ID/구성만 배포한다. 기본·개인/게임/오류의 차단·cache 격리를 유지한다. |
| WEB235-06 | 끝나지 않은 HTTP 요청이 있어도 시험 서버 teardown은 연결을 종료한다. 기존 기능 단언·10초 업데이트·시간 제한/재시도는 완화하지 않는다. |
| WEB235-07 | 최신 CI 모바일 두탭의 ACK 이후 Warm Up/old 재시작 자료를 보존하고 원인을 구분한다. 실제 대조 없이 원인 해결·재시도0·출시 준비 완료로 표시하지 않는다. |

## 현재 관측과 실행 기록

#231 CI의 브라우저 오류는 기능 단언 뒤 시험 fixture teardown이었다. 고정 Content-Length보다 작은 body를 보낸 실제 HTTP 연결의 종료300ms 실패를 Red로 재현했고, 시험 서버가 active 연결까지 닫는 수정안에서 같은 검사가 통과했다. timeout/retry 변경이 아니다.

#233 CI의 다른 실패는 모바일 두탭 update의 load10초 초과다. 완전/잘림 없는 native trace에 Warm Up1과 old worker stopping→stopped→starting→running이 있고, 두 prepare ACK 뒤 skipWaiting-request가 끝나지 않았다. 첫 실패와 같은 것으로 합치지 않으며 추가 대조 후 수정한다.

구현과 검증 중이다. 실제 Google 계정·법적 문구·도메인·장비 검수는 수행하지 않았다.


## 실행 중 확인한 추가 회귀

실제 배포의 service-worker 설치가 완료되지 않아 조사한 결과, 배포 이미지의 `/index.html?__WB_REVISION__=fixture`는404였다. 빌드 manifest의 물리 HTML 주소를 실제 공개 웹 주소로 변환하고 fallback의 캐시 키를 `/`로 통일했다. Google모드 navigation이 일반 precache route보다 먼저 네트워크/오프라인 fallback을 선택하도록 등록 순서도 맞췄다. HTML alias나 기존 개인/query 차단을 풀지 않는다. 실제 Nginx의 worker activated/controller를 별도로 검사한다.

현재 로컬 기록: web478/69files, Rust workspace/Clippy, PostgreSQL233(새 Google 제출 포함), Playwright163(retry0, 최종 precache 수정 전), overlay 반복24(retry0). 실제 배포 최종검사·최신 CI/coverage는 아직 진행 중이며 이 기록만으로 병합·운영 완료를 판정하지 않는다.

시험 서버의 실제 root HTML 검사는 빌드된 `dist`를 사용하므로 `yarn build` 뒤 `yarn test:worker-server`를 실행한다. CI에서 이 순서가 빠져404가 났던 실행은 제품 성공으로 계산하지 않는다.

실제 구성된 Google 시험 이미지의 Chromium 검사도 통과했다: 최초 pause1, 현재 선택 뒤0, 공개 동의창 재열기·광고0, worker activated/controller, 게임/개인 새 문서의 태그0, 실제 게시 정책·ads.txt,304 없는 CSP nonce/본문 일치. 이는 합성 SDK 응답의 연결 검증이며 실제 Google 서비스 승인이 아니다. 기본 Nginx56문서·WASM·worker 활성화도 통과했다. 최종 root fixture 수정 뒤 PC/mobile offline12건은 재시도0으로 통과했다.

중간 CI2589ac5는 Rust/core coverage/실DB/백업이 통과했다. 계측 server95.13%(13016/13683), Consent domain99.46%(370/372),9개domain 모두95%이상이다. Web은 build전fixture의404로 실패했으므로 순서를 수정해 최신head에서 다시 검증한다. 최신 exact head의 전체checks·PR리뷰·병합은 이와 별도다.
