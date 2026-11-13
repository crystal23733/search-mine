# 출시 전 운영자 작업

코드 구현·가짜 SDK/시험 계정·Docker fixture 검증과 실제 공개 서비스 검수는 구분한다. 이 목록은 실제 입력과 관측이 필요한 출구다. #235의 코드 검증 결과는 [검증 기록](../verification/235-public-launch-wiring.md)을 확인한다. 비밀값은 채팅·이슈·PR에 올리지 않는다.

## 실제 서비스 입력

1. #24: 도메인, 미니 PC/OS/회선과 사용 약관, Cloudflare Tunnel을 준비한다. [컨테이너 배포](container-deployment.md)를 따라 공개 포트 없이 운영한다. 서버·DB 주소를 공개하지 않는다.
2. #42: Google·Apple·카카오·네이버 앱 등록과 실제 HTTPS callback을 설정한다. [외부 입력](external-inputs.md)에 있는 최소 권한과 실제 로그인·연결 해제·Apple 철회/알림을 확인한다. 제공자 비밀은 보호된 호스트 파일로만 전달한다.
3. #147: 실제 운영자·연락처·보관/이전/미성년자 처리와 개인정보/약관의8언어 문구를 확정한다. `deploy/public-site.example.json`을 외부 보호 경로에 복사해 채운다. 빈 예시는 게시할 수 없다. 본문은 HTML이 아닌 일반 텍스트이며 공개 JSON/웹 문서로 게시된다.
4. #147: AdSense 실제 publisher/사이트 승인과 자동광고를 설정하고 Google 개인정보 보호 및 메시지를 게시한다. 개인정보 URL은 `https://실제도메인/{locale}/privacy`, 관리 URL은 `/{locale}/consent`다. 자체 목적(분석1/8/10·광고측정1/7)을 실제 문구와 일치시킨다. 적용 지역을 확인하지 못한 경우 광고는 차단된다. 실제 태그에서 관측한 frame/connect/image HTTPS origin을 공개 구성의 `google` 목록에 넣고 CSP/쿠키/네트워크를 검수한다. 임의 wildcard나 script unsafe-inline/eval은 추가하지 않는다.

## 이미지와 공개 문서

공개 구성은 credentials를 넣는 파일이 아니다. 실제 법적 문구와 CSP origin만 넣는다. 내용 변경 시 빌드 캐시가 갱신되도록 전용 명령이 SHA-256과 BuildKit mount를 전달한다. 기존 운영자 파일은 Docker context에 복사하지 않는다.

```sh
python3 deploy/build_web.py --origin https://실제도메인 --tag 실제이미지태그 --config /호스트/경로/public-site.json
```

광고를 활성화할 때만 같은 명령에 `--publisher 실제-ca-pub-ID`를 추가한다. 정책/CSP/ID/공개 origin이 없거나 잘못된 구성은 Google모드 빌드가 실패한다. 광고를 켠 이미지에 맞춰 호스트 `deploy/.env`의 `VITE_CMP_PROVIDER=google`과 실제 `VITE_ADSENSE_CLIENT_ID`를 설정한다. 광고 없이 시작하면 둘 다 빈 값으로 둔다. Compose의 `LIAR_WEB_IMAGE`는 검토한 해당 이미지의 immutable digest를 사용한다. `compose up --build`로 검토한 이미지를 다시 만들지 않는다.

Google모드는 실제 publisher에 맞는 `/ads.txt`를 생성한다. [Google 공식 ads.txt 안내](https://support.google.com/adsense/answer/12171612?hl=en-EN)의 계정 내 값과 공개 응답을 대조한다. 가짜 SDK 시험 통과는 실제 광고 승인·표시 성공이 아니다. 동의 전 관련 script와 기존 Google 쿠키 읽기가 가능하지만 광고 요청은 pause 상태다.

## 장비에서 남는 확인

- #25: 별도 매체와 age 키 보관, 백업/권리 checkpoint timer 및 receipt, 이전 release에서의 격리 복원과 삭제/보존 재적용, 실제 RPO/RTO를 기록한다. 원본 중단·최신 비교 없이 복구를 재개하지 않는다.
- #26: 실제 미니 PC에서 동일한 깨끗한 release로 부하를 반복해 제한과 p95를 정한다. 로컬 fixture의 수치를 운영 동접 한도로 쓰지 않는다. `deploy/preflight.py`와 `deploy/release_report.py`의 실제 증거를 채운다.
- #223: 실제 HTTPS에서 Android/iOS/데스크톱 PWA 설치·실행·업데이트와 오프라인 연습을 확인한다. 8언어/화면낭독기/키보드 검수는 사람이 확인한다.
- #27: 실제 참가자의 두탭/모바일/대전 관찰을 진행한다. #28의28일 수익·전력·잔존 지표는 출시 후 기간이 지난 뒤 기록한다. 이를 미리 달성한 것으로 표시하지 않는다.

완료 증거가 없는 부모 이슈와 실제 운영자 이슈는 닫지 않는다. 내부 운영은 승인된 호스트 전용 CLI로 수행하며 공개 운영자 API/포트는 만들지 않는다.
