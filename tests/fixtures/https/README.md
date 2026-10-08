# HTTPS 브라우저 시험 인증서

localhost 전용 공개 시험 키다. 실제 서비스/Cloudflare Tunnel/OAuth 키로 사용하지 않는다.
자체 서명 인증서이며 Playwright fixture만 ignoreHTTPSErrors를 사용한다.
auth_fixture example은 127.0.0.1:3001, 시험 프록시는 127.0.0.1:8443에서만 수신한다.
production main/config는 시험 provider나 clock control을 선택할 경로가 없다.

생성: openssl req -x509 -newkey rsa:2048 -nodes -keyout localhost-test-only.key -out localhost-test-only.pem -days 36500 -subj /CN=localhost -addext subjectAltName=DNS:localhost
