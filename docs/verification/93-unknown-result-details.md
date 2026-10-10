# #93 미확인 결과 계약 검증

M3 · FR11/14/16, NFR01/02/05/06/07/08 · TS15/16/20/21/26/31/35/36. [ADR0042](../adr/0042-unknown-result-details.md). 기준 develop8347efa, 선행91/PR92 병합·종료.

## 검증 계획과 범위

Rust projection/HTTP·forward migration·실제 PG upgrade/제약/조회/충돌/삭제, strict decoder/UI와 PG/HTTPS PC/mobile를 행동 Red→Green으로 검증한다. 실행 결과는 실제 완료 후 기록한다. unknown SQL fixture는 계약만 검증하며 실제 journal/admission/startup/processkill은 후속18이다.

## 실제 Red와 관련 Green

변경 전 strict decoder의 unknown 양성 시나리오는 malformed로 실패했고 기존 두 테스트는 통과했다. 실제 PG17.4의 unknown 결과 저장은 end_elapsed_ms NOT NULL/23502로 실패했다. 첫 Rust 실행은 잘못된 exact 필터로 실행0이었으며 성공 검증으로 세지 않고 필터 수정 뒤 실제 실패를 확인했다. 원천 Option/생성 TS·새 migration·reader/project·decoder/UI 구현 뒤 같은 decoder3개와 실제 DB unknown 테스트가 통과했다.

추가 회귀 검증: 실제 PG 결과6개/ignored0(0.88초)는 이전 migration202610090002에서 known 결과를 저장한 뒤 forward migration으로 JSON을 그대로 보존하고,16개 null패턴·타 reason/outcome 제약·부모/자식 불일치 거절·양 인간/bot·다른 계정 부재·immutable writer 충돌·삭제 retry 비복원을 확인했다. 서버 results unit7개와 인증 HTTP14개(0.93초), 웹 queue/decoder8개(5.70초)·typecheck/lint가 통과했다.

전체 검사에서 Rust fmt/Clippy/workspace native/WASM/생성 TS/native fixture/token 검사는 통과했고 번역 JSON8개의 포맷에서 중단됐다. 해당 JSON만 Prettier로 정리한 뒤 남은 format/lint/typecheck/전체 웹 단위/빌드/브라우저/문서 검사를 이어 실행한다. 전체 웹199개/35파일10.33초·line87.94% 통과. 아직 끝나지 않은 브라우저/서버 전체 coverage/원격 CI는 완료로 세지 않는다.

## 코드 리뷰 경계

pm-ai-shipping:code-review의 correctness를 기준 develop8347efa 대비 변경에 적용한다. DB null/제약→Rust 변환/project→인증 전후 proof→closed decoder→controller generation/request identity→Atomic UI의 값을 따라간다. live timeout/win 후보와 실제 stored server_failure/abort/false가 달라도 서버 결과를 표시하는 테스트로 권위 일치를 확인한다. 기존 역순 요청·WS 우선·철회/삭제·취소와 duplicate writer의 참여자 비복원도 함께 검토한다. 최종 검토 결과와 CI/PR은 실행 후 기록한다.

## 최종 로컬 결과와 검토

첫 전체 HTTPS 실행은 공용 fixture DB에 새 migration을 적용하지 않아 unknown seed가503으로 실패했다(70통과/1실패/19미실행). 격리 schema의 upgrade 통과와 구분하고 trace를 로컬 .tmp/result93-before-migration-trace.zip에 보존했다. MIGRATION_DATABASE_URL을 사용한 정식 migrate 명령 적용 후 신규1440/390px2개14.1초, 전체90개2.7분·재시도0로 통과했다. 실제 PG17.4·운영 reader/router의 no-store 최소 응답, 본인200/다른 계정 uniform404,8언어 abort/통계 확인 불가·숫자/분모/진행률 미생성·일반 storage/광고 요청0, 다시하기를 검증했다. 두 화면 screenshot도 직접 확인했다. 문서143/Mermaid34실패0.

서버 cargo llvm-cov의 실제 DB32개/ignored0(3.27초)·HTTP14개(0.94초) 포함 전체 검증 통과. Auth385/387=99.48%, Match287/289=99.31%, Lobby362/368=98.37%, Result58/58=100%, 전체6166/6646=92.78%로95/80 기준을 충족했다. 테스트/예제 경로를 제외하고 실행 entry point를 포함했다. 웹 line2277/2589=87.94%다.

최종 correctness 리뷰에서 explicit null/미지정/0 구분, SQL CHECK의 null 통과 방지·부모/자식 fail-closed, immutable writer·삭제 비복원, known JSON/과거 hash 유지, 서버 completion·동일 match·전후 세션 proof, 요청 역순/폐기·WS 우선·UI 권한을 확인했다. fixture transaction의 부모/자식 원자성과 production writer 미활성 경계를 함께 검토했다. 검토 범위에서 근거가 있는 미해결 finding은 없다. #83 원인·실제 journal/startup/processkill·외부42/26/27·보안/성능 전체 감사를 완료했다고 주장하지 않는다. 원격 CI·PR/병합은 후속 기록한다.
