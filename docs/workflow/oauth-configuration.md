# OAuth 운영 설정과 별도 검수

#44 서버 계약 설정이다. 실제 키·등록 도메인·제공자 승인·실계정 검수는 [#42](https://github.com/crystal23733/search-mine/issues/42)에 남긴다. 실제 키를 Git/이슈/PR/채팅에 붙이지 않고 홈서버의 secret 저장소에 넣는다. 연결/삭제/Apple credential 처리는 #45, 화면은 #46이다. 둘의 완료 전 공개 OAuth를 출시하지 않는다.

## 공통

`LIAR_PUBLIC_ORIGIN`은 path/query/userinfo 없는 실제 HTTPS 도메인 origin이다. origin이나 인증 이름이 하나라도 설정되면 세 공통 키를 전부 요구하고 부분 설정을 거절한다. 모든 인증 이름이 없으면4개 비활성·계정 null/CSRF null로 로컬 연습/health를 유지한다. 인증 설정을 켤 때 DML `DATABASE_URL`도 필요하다. migration CLI는 별도 DDL 계정의 `MIGRATION_DATABASE_URL`을 쓰며 serving에서는 DDL 권한이 없다.

- `LIAR_AUTH_CSRF_KEY`: CSPRNG32byte를 padding 없는 base64url로 저장한다.
- `LIAR_AUTH_DIGEST_KEYS`, `LIAR_AUTH_VAULT_KEYS`: 서로 다른32byte 키의 JSON keyring. 형식은 `{"current":1,"keys":[{"version":1,"key":"<base64url32bytes>"}]}`이고 최대4개다. 0키/미등록 current/중복 version/잘못된 길이를 거절한다. DB 밖에 보관하고 앱/백업 키를 분리한다.
- digest current를 바꿀 때 모든 쓰기 프로세스의 current를 같게 유지한다. identity 쓰기를 잠시 멈추고 동기화한 뒤 재개하며 이전 키를 즉시 제거하지 않는다. [인증 설계](../design/17-auth-privacy.md).
- `__Host-liar_session`과 `__Host-liar_browser`는 Secure/HttpOnly/Lax/Path=/이다. 브라우저에서 실제 로그인하려면 HTTPS를 제공해야 한다. HTTP 개발 환경을 위해 보안 cookie를 완화하는 옵션은 없다. 테스트는 별도 DI composition을 사용하고 운영 인증 우회 URL은 없다.

## 네 제공자

아래 client ID/secret 쌍이 모두 없으면 해당 제공자만 비활성이다. 일부만 있으면 시작 실패다. URL을 임의 입력하는 설정은 없으며 authorize/token/JWKS/profile은 코드의 공식 HTTPS allowlist를 사용한다. redirect를 따라가지 않고 요청10초/응답64KiB(JWKS128KiB/64keys)로 제한한다.

| 제공자 | secret 설정 | 등록 callback·최소 권한 |
|---|---|---|
| Google | LIAR_GOOGLE_CLIENT_ID, LIAR_GOOGLE_CLIENT_SECRET | `{origin}/api/v1/auth/google/callback`; openid만, S256; 실제 openid-only 상호운용을 #42에서 확인 |
| Apple | LIAR_APPLE_CLIENT_ID, LIAR_APPLE_TEAM_ID, LIAR_APPLE_KEY_ID, LIAR_APPLE_PRIVATE_KEY_FILE | `{origin}/api/v1/auth/apple/callback`; Services ID, name/email scope 없음, code/query;10자리 team/key ID와 read-only P8 secret mount |
| Kakao | LIAR_KAKAO_CLIENT_ID, LIAR_KAKAO_CLIENT_SECRET | `{origin}/api/v1/auth/kakao/callback`; console OIDC/secret 활성화, openid만·S256; 이메일/이름/사진 등 동의 항목 미설정 |
| Naver | LIAR_NAVER_CLIENT_ID, LIAR_NAVER_CLIENT_SECRET | `{origin}/api/v1/auth/naver/callback`; 추가 제공 항목 미선택·id만, state/token POST; profile의 나머지 값은 버림 |

Apple assertion은 요청별 ES256·5분이며 provider refresh credential은 #45의 AEAD 저장/철회에만 사용한다. 공개 DTO·이벤트·로그에 credential/subject/이메일/실명/사진을 넣지 않는다. Cloudflare와 역프록시 운영 로그도 callback query/code/state와 cookie를 기록하지 않도록 운영 배포 #24에서 설정한다. JWT header의 jku/x5u는 거절하고 외부 URL을 따라가지 않는다.

## 계정 권리와 Apple 유지보수

`LIAR_APPLE_NOTIFICATION_AUDIENCE`는 실제 등록 알림의 정확한 audience다. 없으면 알림 endpoint는503으로 비활성이고 Services ID에서 자동 추정하지 않는다. 값이 잘못되거나 Apple 설정 없이 일부만 있으면 시작을 거절한다. `{origin}/api/v1/auth/apple/notifications`를 console에 등록하고 실제 audience와 이벤트/재시도를 #42에서 확인한다.

Apple refresh는 identity UUID를 AAD로 AEAD 저장한다. 런타임 worker는60초마다 최대8개 철회와8개 일일 확인을 처리한다. 철회 요청10초·확인20초 deadline, queue lease60초·backoff·24시간 삭제를 적용한다. 일반 credential은 하루 한 번만 확인하며 outage/invalid_client/서명 오류는 계정을 삭제하지 않는다. 네트워크가 없어도 로컬 탈퇴는 원자 완료되고 credential 부재/Apple 비활성은 수동 연결 철회를 안내한다. 백업 복구 후28일 tombstone을 적용하는 실제 운영 절차는 #25에서 검증한다.

## 검수 한계

로컬 서명 fixture/HTTP 계약/CI PostgreSQL 검증은 실제 외부 계정 인증을 대신하지 않는다. 제공자별 앱 등록, domain/redirect 승인, 키 발급, 한국/글로벌 실계정 로그인·취소·명시 연결·재인증·삭제/Apple 철회는 실제 환경에서 확인해야 한다. 개인정보 처리 근거/고지·운영자·미성년자 방침 등은 #21에서 확정하며 OAuth 사용만으로 법적 준수를 주장하지 않는다.
