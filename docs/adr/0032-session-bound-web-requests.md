# ADR0032: 온라인 화면의 세션 인증 요청 포트

2026-10-09 · 승인된 #17/FR01/06/07/14의 구현 구체화 · #66/PR67 병합·종료 뒤 #68을 순차 진행한다.

## 결정

기존 AuthPort의 계정 쓰기 operate는 working 전환·generation/revision 변경·계정 안내를 관리한다. 로비 status를 이 경로로 반복 조회하면 매번 온라인 권위가 끊긴다. 별도 AuthenticatedRequests DI 포트의 execute가 예상 accountId/revision과 현재 메모리 account/session/generation을 캡처하고, 안정된 인증에서는 계정 상태·working·revision·안내를 바꾸지 않는다. 로그인/계정 권리 경로는 유지한다.

서버의 session-bound CSRF는5분 수명이다. 각 요청 전에 bootstrap으로 같은 account와 session_revision을 대조하고 새 CSRF를 요청 callback의 메모리에서만 사용한다. 성공 응답 뒤 다시 bootstrap으로 소유권을 확인하고 나서 반환한다. 그 사이 실제 계정/세션·generation/revision이 바뀌거나 offline·취소가 발생하면 응답을 버린다. 닉네임 없는 계정은 온라인 요청을 시작하지 않는다. proof·service cookie·private session UUID를 AuthState/스토리지/URL/로그에 추가하지 않는다.

refresh/invalidate/계정 쓰기/dispose는 기존 generation을 변경하며 해당 권한의 요청을 모두 취소한다. 서로 다른 정상 조회는 서로를 취소하지 않는다. 개별 caller AbortSignal은 자기 요청만 취소한다. 요청 전체는10초, 동시 요청은8개로 제한하며 타이머·abort listener·slot을 성공/실패/취소에 반환한다. transport가 AbortSignal을 무시하더라도 늦은 bootstrap 뒤 대상 요청을 시작하거나 늦은 응답을 성공으로 반환하지 않는다.

대상 domain 오류는 원형 그대로 전달하며 로그인 상태를 지우거나 자동 재시도하지 않는다. 요청 capacity/timeout/caller cancel/stale는 안정된 요청 오류다. bootstrap의 실패·권한 불일치 또는 대상의 명시적 auth_required만 캡처한 권한이 여전히 현재일 때 무효화한다. 오래된 실패가 새로운 계정을 지우지 않는다. 서버가 이미 처리한 효과를 취소로 되돌린다고 표현하지 않으며 후속 로비 controller는 status로 실제 상태를 다시 확인한다.

## 검증과 한계

TS10/12/15/20/31/36을 실제 AuthPort와 통제된 bootstrap/target port로 검증한다. 안정된 병렬 요청과 fresh proof·전/후 계정 및 같은 계정의 session 교체·revision·offline/무계정/닉네임 없음·caller abort·권한 변경/계정 쓰기/dispose·signal 무시 late reply·상한/정확 deadline·domain 오류 보존·새 계정 보호가 Red→Green 대상이다. AuthState 직렬화에 proof/credential/session 정보를 추가하지 않는다.

이 포트만으로 온라인 화면이나 재접속을 완료했다고 보고하지 않는다. 실제 로비 HTTP decoder/WS·8언어 화면·HTTPS/DB 통합은 다음 #17 하위 작업이다. pre/post bootstrap의 추가 RTT/서버 read 비용과 미니PC 용량은 #26의 실측 대상이다. 자동 polling 간격/중복 요청 억제는 다음 controller가 결정하고 Rust rate/admission을 우회하지 않는다. 기존 실키 #42·사람 관찰 #27은 별도다.
