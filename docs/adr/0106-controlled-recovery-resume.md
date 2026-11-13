# ADR0106: 원본 차단을 동반하는 명시적 복구 재개

상태: 채택. 기존 FR16/NFR07 복구·롤백 범위의 운영 경계를 완성한다. #231, 상위 #24/#25/#26. 실제 장비와 계정 설정은 운영자가 검수한다.

## 결정

일반 checkpoint의 `matched`는 재개 권한이 아니다. `resume-from-source`는 인증을 마친 암호화 checkpoint, 별도의 살아 있는 원본 owner 연결, 현재 복구 incident를 모두 요구한다. 두 DB는 외부에 포트를 공개하지 않으며 운영자 복구 작업만 두 내부망에 연결한다.

원본의 journal 배타 잠금을 얻어 실행 중인 writer를 거절하고 원본 gate/binding/source와 checkpoint의 전체 유효 권리 집합을 다시 비교한다. 성공하면 원본 gate를 `sealed`로 commit한다. sealed 원본은 재시작·일반 quarantine 명령으로 활성화할 수 없다. 원본 변경을 중단한 뒤에만 복구 대상을 정리·재개한다. 이는 모든 권리 변경을 동기 복제하는 기능이 아니다.

대상은 같은 source UUID·현재 DB/schema·명시 incident의 quarantine 상태여야 한다. 기존 원장 적용을 재사용해 삭제/연결 해제와 모든 복구 세션·거래·Apple credential을 정리한다. 기존 bounded retention adapter로 원시30일·익명 원래 만료·seed/replay7일·전적90일 정리를 끝내고, 마지막 owner 잠금/incident 확인 후에만 active로 전환한다. 정리 중 잠긴 행·시간 초과·원장 누락은 quarantine을 유지한다. 이후 정상 시작의 기존 journal 복구가 중단된 판을 처리한다.

단계 사이 장애 시 원본 sealed/대상 quarantine은 안전한 중간 상태다. 같은 원본과 원장으로 재시도할 수 있다. 대상 active 이후 응답 유실은 운영자가 gate/readiness를 확인하며 이미 active인 대상에 재적용하지 않는다. DB 관리자에 의한 직접 SQL/복사나 split-brain 강제 우회는 지원하지 않는다. source control DATA를 제외하는 기존 백업 규칙은 sealed에도 적용된다.

원본을 잃고 최신 권리의 완전성을 확인할 별도 자료도 없으면 자동 재개하지 않는다. 운영자는 원본/독립 최신 자료 확보 또는 기존 개인정보를 되살리지 않는 새 설치를 선택한다. 도구가 과거 checkpoint만으로 완전성을 만들어내지 않는다. 실제 RPO24시간/RTO2시간·별도 매체·이전 이미지 훈련은 실측 전 미확인이다.

## 함께 마무리할 배포 도구

부하 도구는 격리된 fixture에서 실제 제품 인증·매칭·WS 포트를 사용한다. 테스트용 제공자만 대체하고 운영 바이너리에는 우회 인증을 넣지 않는다. 성공/거절/오류를 구분하고 지연 표본·실행 환경·번들을 기록한다. admission은 실제 안정 용량 측정의70% 이하로 산정한다. 개발 PC 결과를 미니 PC나 글로벌 RTT 결과로 부르지 않는다.

배포 전 점검은 secret 내용을 출력하지 않고 확정 origin·불변 이미지·secret 파일 권한·0 host port와 명시 운영 증거를 검증한다. 외부 설정이 없으면 준비 완료를 선언하지 않는다. 자동화 가능한 점검과 실계정/실기기/법적 고지/실제 장비 검수를 구분한다.

## 검증

[인수 시나리오](../verification/231-release-operations.md), 기존 [격리](../workflow/recovery-quarantine.md)·[암호화 원장](../workflow/encrypted-rights-ledger.md)과 연결한다. 실제 Red→Green·DB/WS·Docker 결과는 구현 후 기록한다.
