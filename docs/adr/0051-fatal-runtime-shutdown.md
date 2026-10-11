# ADR0051 · 치명 오류와 blocking runtime 종료

- 상태: 채택 · 2026-10-11 · M3 #109 / 상위 #19
- 추적: NFR01/02/05/07/08 · TS20/21/31/35/36
- 선행 관측: PR110 첫 Product38112810532의 실제 PG18.6 검사에서 기존 main owner-loss 검사가 프로세스 종료를4초 안에 관측하지 못했다. 60pass/1fail이며 stderr가 비활성인 기존 fixture만으로 종료 지연의 내부 원인을 확정할 수 없다.

## 경계와 결정

현재 `#[tokio::main]`이 만드는 runtime은 async main의 오류 반환 뒤에도 Drop으로 종료한다. 기존 BoardPool과 새 replay worker는 spawn_blocking을 사용한다. Tokio의 [Runtime shutdown 계약](https://docs.rs/tokio/latest/tokio/runtime/struct.Runtime.html#shutdown)은 이미 시작한 blocking 작업이 끝날 때까지 Drop이 기다린다. CPU 작업이 걸린 상태에서 저장 소유권 상실을 감지해도 프로세스의 치명 오류 반환을 지연할 수 있는 경계다. 이것이 첫 원격 실패의 유일한 원인이라는 주장은 하지 않는다.

실행 진입점을 동기 main과 async serve로 나누고 runtime의 소유권을 진입점에 둔다. async serve가 오류를 반환하면 `shutdown_background`로 runtime I/O를 종료하고 같은 오류를 main으로 반환하여 프로세스가 실패 종료한다. 정상 성공은 기존 Drop/graceful 경로를 따른다. 이 함수는 단일 실행 파일의 끝에서만 사용하며 요청별 runtime 생성이나 복구/재사용에 사용하지 않는다.

background shutdown은 blocking 코드를 강제 중단하지 않는다. worker가 끝나거나 프로세스가 끝날 때까지 그 자원은 살아 있을 수 있다. 따라서 요청 timeout/cancel 뒤 actual permit을 유지하는 ADR0050/BoardPool 계약을 바꾸지 않는다. 프로세스 종료 뒤 OS가 연결·thread를 해제하며 새 인스턴스는 기존 durable journal 복구 경로를 사용한다. 사용자/secret 로그나 새 환경변수·endpoint를 추가하지 않는다.

## 행동 검사

실제 BoardPool에 gate로 멈춘 SeedSource를 주입한다. actual blocking worker 진입을 관측한 뒤 serve가 치명 오류를 반환한다. gate를 열기 전에 진입점이 오류를 반환해야 한다. 기존 Drop 경로에서 실제 Red를 확인하고 수정 뒤 Green을 확인한다. 테스트는 실패해도 gate를 열고 thread를 join하여 프로세스를 남기지 않는다. 일반 오류 전달과 정상 성공도 검사한다. 실제 PG main owner 상실·기존 HTTP/WS 종료 검사와 전체 gates를 다시 실행한다. 기존4초/요청2초 제한과 retry를 늘리지 않는다.
