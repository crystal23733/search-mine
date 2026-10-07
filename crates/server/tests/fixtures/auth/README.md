# 인증 테스트 전용 키

이 디렉터리 RSA/P-256 private/public key는 이슈 #44의 서명 fixture를 위해 로컬에서 새로 생성한 공개 테스트 데이터다. 실제 OAuth 앱/사용자/운영 키가 아니며 운영 설정에서 사용하면 안 된다. JWKS는 RSA 공개 modulus/exponent만 포함한다. 고정 시험 키로 검증 성공과 변조/alg/issuer/audience/nonce 거절을 재현한다.
