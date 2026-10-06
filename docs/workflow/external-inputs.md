# 외부 입력·실측 준비 목록

사용자가 전체 순차 작업과 병합을 위임했다. 아래 입력이 필요한 부분만 후속 확인 항목으로 남기고 독립적인 구현·계약 테스트·CI는 계속한다. 키를 GitHub 이슈/PR/채팅/커밋에 넣지 않고 사용자 host secret 파일/환경에 설정하도록 안내한다. 임시 키나 운영자 개인정보를 만들지 않는다.

| 시점 | 사용자 준비 | 구현 가능한 부분 / 남는 확인 |
|---|---|---|
| #15 Google | OAuth web client ID/secret, 등록된 HTTPS callback | code/PKCE/OIDC adapter·mock 계약·세션 구현; 실제 계정 로그인 검수는 키 설정 후 |
| #15 Apple | developer team·Services ID·key ID/private key·등록 HTTPS 도메인 | code/query·ID token·credential vault/revoke/알림 구현; 실제 계정·알림/철회 검수는 설정 후 |
| #15 Kakao | REST app key/client secret, OIDC 활성화·openid 외 동의 항목 해제 | adapter·최소 claim·계약 테스트; 실제 사용자 동의 화면 검수 |
| #15 Naver | client ID/secret·검수 앱·callback, 기본 id 외 제공 항목 해제 | code/state/profile id adapter·계약 테스트; 사전 검수/실계정 확인 |
| #21 정책/CMP | 실제 운영자/연락처·국외 이전/미성년자 처리 방침·인증 CMP 계정 | 선택 동의·고지·번역/거부 경로; 확정 법적 고지·실CMP 검수 |
| #22 광고 | 실제 publisher/slot ID·서비스 승인 | 무광고 기본·AdsPort cap/실패 테스트; 실제 광고·ads.txt는 승인 후 |
| #24 홈서버 | 도메인·Tunnel token·미니 PC/OS/회선 정보·host secret | Compose/Tunnel/배포 runbook·격리·CI; 실제 호스트 공개/회선 약관 확인 |
| #25/26 운영 | 별도 백업 매체·미니 PC·실측 네트워크 환경 | 격리 복구/부하 harness; 해당 장비 RPO/RTO/동접/전력 결과 |
| #27/28 베타 | 실제 참여자·28일 정산/전력·사용 지표 | 테스트 가이드/분석 도구; 사람의 재미 관찰·현금 손익 결과 |

키가 없을 때 비활성 제공자를 성공 응답으로 대체하거나 OAuth 설정을 UI에 노출하지 않는다. 공개 버전의 필수 제공자 4개 연동·정책·운영 검증 게이트는 실제 확인 전 완료로 표시하지 않는다. 후속 사용자 입력 이슈가 생기면 이 표에 연결한다.
