# #107 · 양쪽 이탈과 재접속 통합 출구

[ADR0049](../adr/0049-reconnect-integration-exit.md) · FR14/16 · NFR01/02/05/06/07/08 · TS15/16/20/21/26/31/35/36. 선행 #105/PR106 develop1f840bf 병합·#105 CLOSED를 확인하고 시작했다.

## 실제 시나리오와 보정

관측 probe 없는404 대200의 실제 Red를 확인했다. 시험 전용 proxy에 전역 transport accepted/server_ended/pending/forced 수만 읽는 고정 GET을 추가했다. client 종료는 write half를 end하고 서버 EOF를 기다리며2초 뒤 남은 연결만 정리한다. forced/error/client close를 server_ended로 세지 않는다. 운영 Rust/웹 source·규칙/DTO/schema/의존성 변경은 없다.

최초 Green 시도는 실제 abandoned/draw/known own33초를 저장했지만 새 테스트의 completed:false 기대 때문에 실패했다(3.3초). 승인된 규칙04·PublicEndReason.completed·기존 result model은 abandon true, 서버 장애/시작 전 취소 false다. 제품 결함으로 분류하지 않고 코드 전에 새 ADR/추적/이슈 기대를 기존 계약에 맞췄다. 다음 시도는 ready 화면에 없는 재시도 버튼 기대에서 기본30초로 실패했다. 기존 고정 results reload로 바로잡았으며 timeout/retry를 늘리지 않았다. 실패 trace/log는 ignored .tmp에 보존했다.

보정 후390/1440px 두 실제 시나리오가16.0초/재시도0 통과했다. 실제 OAuth fixture 계정 두 개·HTTPS pair·WS playing 뒤 인증쿠키만 저장하고 두 context를 동시에 닫았다. 같은 clock을 멈춘 채 upstream EOF+2/pending0/forced증가0을 확인한 뒤30초 진행했다. 실제 SQL 저장을 관측한 후 새 context의 고정 results에서 ID없는 POSTv1·최소 본인 결과를 확인했다. known own/elapsed·abandoned/draw/completed:true,8언어·가로넘침0·일반storage ID없음·자동 WS/lobby 재입장0을 검사했다. active0/final1/입장 anchor/known players·추가240초와 actual OS kill→새PID→ready 후 결과/시각/통계 불변도 확인했다. 최종 전체 회귀에서도 한 사람만 실제 safe open하여 본인 통계5 대4를 구분했고 두 계정의 fresh lookup·OS restart·reload 결과가 각각의 원래 통계와 일치했다.

## 상위18 인수 조건별 감사

| #18 조건 | 구현/시나리오 근거 | 실제 실행 근거와 한계 |
|---|---|---|
| 30초 서버 유예·새 epoch·권위 snapshot·진행 시간 유지 | [74 cursor](74-reconnect-command-cursor.md), [76 grace](76-opponent-reconnect-grace.md), [80 recovery](80-bounded-online-reconnect.md), core/online_state/online_actor | HTTPS online-reconnect의 peer disconnect→resume→SQL forfeit와 reload→last_client_seq+1을390/1440px에서 검사.105최종 전체94browser에서 기존 시나리오 재검증. |
| UUID 재전송 원본 ACK·중복 flag/gauge/통계 없음 | [80](80-bounded-online-reconnect.md), online-recovery.ts의 실제 적용된 ACK1개만 유실·재연결/원본UUID/seq/action/revision 유지·epoch만 교체 | 실제PG/WS/HTTPS PC/mobile 재전송·cachedACK/원래revision·flag1회·다음seq 및 offline 쓰기0/fresh proof. reload 중 잃은 미확인 명령 복원은 지원하지 않는 명시 계약. |
| 한 명 forfeit/둘 abandon/OS restart abort | [76](76-opponent-reconnect-grace.md), 본107, [103](103-durable-lobby-admission.md), [105](105-latest-personal-result.md) | forfeit win/true 실제HTTPS/SQL; 양쪽 abandoned draw/true는 새 실제EOF→SQL→fresh latest;103 실제main OSkill/unknown abort와105새browser4개는 server_failure/abort/false·null통계·seed없음. 클라이언트 시간으로 결과를 만들지 않는다. |
| 저장1회·pending/saved/failed·완료 계약·secret | [16 actor](16-authoritative-match-actor.md), [82 read](82-authenticated-personal-result-read.md), [93 unknown](93-unknown-result-details.md), [97 owner](97-online-storage-owner.md), [99 anchor](99-result-retention-anchor.md), [101 journal](101-active-match-journal.md), [103](103-durable-lobby-admission.md) | online_actor의 bounded retry 실패/Failed와 실제PG concurrent duplicate·2번째seat rollback·삭제 비복원·owner loss, 실제main recovery failure시 serving 거절/부분저장0.105실제OS2회·active0/final1. Result model 모든reason 분류/nullable 허용 조합, 최소DTO/secret 검사. 실제 광고/승률 소비20/23 구현 완료로 계산하지 않는다. |
| 실제DB/WS/HTTPS PC/mobile·권한/8언어·설계/추적/리뷰/checks | 관련 verification/ADR0035~0049·현재 검사설계/TS15/16/20/21/26/31/35/36 |105CI Product38075053329의PG18.6 61/ignored0·211unit·94browser3.6분재시도0·서버92.97%/domain95이상.107최종 전체 검사/원격CI/코드리뷰는 아래 기록 후 판단한다. |
| 모든 하위 develop병합/이슈종료 뒤18종료 | 다음 표의 GitHub 실제 조회 |107자체 PR병합/이슈종료는 아직 미완료. 충족된 상태 확인 뒤에만18을 종료한다. |

## 실제 GitHub 하위 종료 조회

2026-10-11 Asia/Seoul 기준 원격 issues/PR 일괄 조회에서 선행17 CLOSED(2026-10-09T07:29:29Z), 아래 모두 develop MERGED 및 해당 이슈 CLOSED를 확인했다. 표 시각은 UTC이며 단순 백로그 표기만으로 완료 처리하지 않는다.

| 이슈 / PR | 실제 mergedAt UTC | 실제 closedAt UTC |
|---|---|---|
| [#72](https://github.com/crystal23733/search-mine/issues/72) / [PR73](https://github.com/crystal23733/search-mine/pull/73) | 2026-10-09T07:28:53Z | 2026-10-09T07:28:54Z |
| [#74](https://github.com/crystal23733/search-mine/issues/74) / [PR75](https://github.com/crystal23733/search-mine/pull/75) | 2026-10-09T12:09:19Z | 2026-10-09T12:09:20Z |
| [#76](https://github.com/crystal23733/search-mine/issues/76) / [PR77](https://github.com/crystal23733/search-mine/pull/77) | 2026-10-09T12:42:53Z | 2026-10-09T12:42:55Z |
| [#78](https://github.com/crystal23733/search-mine/issues/78) / [PR79](https://github.com/crystal23733/search-mine/pull/79) | 2026-10-09T13:06:11Z | 2026-10-09T13:06:13Z |
| [#80](https://github.com/crystal23733/search-mine/issues/80) / [PR81](https://github.com/crystal23733/search-mine/pull/81) | 2026-10-09T13:50:59Z | 2026-10-09T13:51:00Z |
| [#85](https://github.com/crystal23733/search-mine/issues/85) / [PR86](https://github.com/crystal23733/search-mine/pull/86) | 2026-10-09T14:34:47Z | 2026-10-09T14:34:49Z |
| [#84](https://github.com/crystal23733/search-mine/issues/84) / [PR87](https://github.com/crystal23733/search-mine/pull/87) | 2026-10-09T15:08:45Z | 2026-10-09T15:08:46Z |
| [#82](https://github.com/crystal23733/search-mine/issues/82) / [PR88](https://github.com/crystal23733/search-mine/pull/88) | 2026-10-10T02:29:45Z | 2026-10-10T02:29:46Z |
| [#89](https://github.com/crystal23733/search-mine/issues/89) / [PR90](https://github.com/crystal23733/search-mine/pull/90) | 2026-10-10T04:34:42Z | 2026-10-10T04:34:44Z |
| [#91](https://github.com/crystal23733/search-mine/issues/91) / [PR92](https://github.com/crystal23733/search-mine/pull/92) | 2026-10-10T04:52:32Z | 2026-10-10T04:52:33Z |
| [#93](https://github.com/crystal23733/search-mine/issues/93) / [PR94](https://github.com/crystal23733/search-mine/pull/94) | 2026-10-10T12:20:25Z | 2026-10-10T12:20:26Z |
| [#95](https://github.com/crystal23733/search-mine/issues/95) / [PR96](https://github.com/crystal23733/search-mine/pull/96) | 2026-10-10T12:39:27Z | 2026-10-10T12:39:28Z |
| [#97](https://github.com/crystal23733/search-mine/issues/97) / [PR98](https://github.com/crystal23733/search-mine/pull/98) | 2026-10-10T13:28:21Z | 2026-10-10T13:28:22Z |
| [#99](https://github.com/crystal23733/search-mine/issues/99) / [PR100](https://github.com/crystal23733/search-mine/pull/100) | 2026-10-10T13:53:20Z | 2026-10-10T13:53:21Z |
| [#101](https://github.com/crystal23733/search-mine/issues/101) / [PR102](https://github.com/crystal23733/search-mine/pull/102) | 2026-10-10T17:04:15Z | 2026-10-10T17:04:16Z |
| [#103](https://github.com/crystal23733/search-mine/issues/103) / [PR104](https://github.com/crystal23733/search-mine/pull/104) | 2026-10-10T17:38:09Z | 2026-10-10T17:38:11Z |
| [#105](https://github.com/crystal23733/search-mine/issues/105) / [PR106](https://github.com/crystal23733/search-mine/pull/106) | 2026-10-10T18:23:04Z | 2026-10-10T18:23:06Z |

현재107은 OPEN/미병합이므로18 OPEN을 유지한다. 최종 CI/head·107 병합/종료 및18 종료는 후속 기록이다. M3 전체(인증15의 실OAuth42와19/20 등) 완료로 계산하지 않는다. 원인 미확정83·실OAuth42·하드웨어26·사람27·물리삭제/백업25는 실제 OPEN이며 이 검증으로 닫지 않는다.


## 최종 로컬 검사와 코드 리뷰

`scripts/check.ps1` 실제 Exit0: fmt/Clippy·Rust315·WASM/types/fixtures/tokens freshness·format/lint/typecheck·211unit37파일·web line88.25%·productionbuild·96browser3.3분/재시도0·docs157/Mermaid34실패0. 별도 실제 PostgreSQL17.4 61개7.90초/ignored0을 통과했다. 신규5/4통계 시나리오2개도 전체 회귀에 포함됐다. source 수정 없이 이 최종 결과를 기록했다. 서버/domain coverage는 제품 source가 같은105의 실제92.97%/각domain95%이상과 이번 원격 CI를 구분하며 새 로컬 측정으로 표현하지 않는다.

전체 suite의390/1440px 관측은 각각 accepted56/58·server_ended48/50·pending0·forced8이다. 앞선 실제OSkill 시나리오들의 forced8을 보존했고 새 양쪽 이탈 시나리오에서 증가하지 않음을 검사했다. isolated 두 시나리오의 forced0과 구분한다. actualOSkill SIGKILL/새PID/currentUTC와33초 known결과/원래anchor·result1 불변 기록은 ignored .tmp/online-layout/abandoned-*.json에 보존했다. 모바일 abandon 화면의 본인5칸·무승부·저장 상태와 넘침 없음을 직접 확인했다.

pm-skills code-review correctness/changes·baseline develop1f840bf로 browser/proxy/WS함수 Drop/직렬 ingress→core→SQL→본인reader/freshcontext의 계약을 검토했다. 요청한 false와 기존규칙/실제응답 true를 반증했으며, 동시 종료·연결 객체/전역counter재사용·oldcontext 제거/새context·실제OS교체와 서로 다른 own5/4로 상관관계를 확인했다. client close를 서버EOF로 세지 않으며 오류/강제정리를 별도 수로 세는지 검토했다. 현재 근거 있는 미해결 범위 내 결함은 없다. review/2026-10-11.md와 실패trace는 ignored이며 커밋에 넣지 않는다. 최신 원격 CI/head·107병합/종료·상위18 종료는 아직 미완료다.


## 최초 원격 CI 실패와 관측 간격 보정

Product38076749329/database114285214822의 일반PG61은 통과했고 coverage 재실행은60pass/1fail/17.13초였다. 기존103 `real_main_os_kill_recovers_authenticated_ws_match_once_with_original_anchor`가 pair helper HTTP429 대200으로 실패했다. 실제 로그 .tmp/abandon107-ci-db-failure.log를 보존했다. 제품 session/account rate는20회/초인데 helper20ms는최대50회/초이므로 준비가 길어지면 한도를 넘을 수 있다. 코드 전에 ADR0049/검사 설계를 보완하고 정상 상태 poll을100ms/최대10회/초로 맞춘다. 제품 rate/capacity와 전체4초 deadline·429거절·timeout/retry는 유지한다. 새 head의관련 실제61PG/정적 검사/전체원격CI를 확인한 뒤에만병합한다. 제품 결함이나 새109작업으로 우회하지 않는다.


준비 poll을 제품20회/초 이하100ms로 보정한 뒤 fmt/Clippy all-targets와 실제PG61개8.62초/ignored0을 통과했다. 변경은 기존 native 시험helper 한 줄과 설명뿐이며 제품source/4초기한/rate/재시도는 유지했다. 직전 전체315Rust/211unit/96browser3.3분재시도0/문서PASS의브라우저source는동일하다. 해당관측수정diff와rate경계를추가리뷰했고최신수정head전체CI를다시확인한다. 최초원격실패를숨기거나성공으로계산하지않는다.
