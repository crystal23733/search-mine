# #58 · 실제 매치 조립

#17/M3 · FR02/06/07·TS02/10/11/12/21 · [ADR0027](../adr/0027-atomic-lobby-match-composition.md) · 선행 #56 PR57/develop8dc226b 병합·종료.

core PreparedEngine은 인증·초기 관측을 미리 준비하고 admission의 서버 시각에 start되어 전체 countdown을 받는다. 기존 new/new_solo의 유효 projection·입력 규칙을 유지한다. private Board/rules/player 상태에는 Debug/Serialize/Clone이 없고 준비 객체는 한 번 소비한다. 최초4개 테스트가0passed/4failed Red(.tmp/lobby58-core-red.log)였고 구현 후4개Green이다.

LobbyService는52 policy·54 BoardSource·56 bot driver와 실제 MatchRegistry를 연결한다. 같은 registry clock/한 Lobby lock에서 live entity+generation을 확인하고 prepared start→registry 성공→policy commit을 같은 시각에 수행한다. 준비 slot은 최대2초의 board take와 actual blocking prepare를 포함하며 실제 closure가 permit을 보유한다. waiting take 취소는 future를 중지하지만 이미 진행 중인 CPU는 끝날 때까지 slot을 보유한다. jobs의 key별 oneshot은 취소/expiry/unready/Drop 후 제거되어 늦은 결과가 새 대전을 만들 수 없다.

최초 조립9개는0passed/9failed Red(.tmp/lobby58-service-red.log)였다. 구현 뒤8passed/1failed였으며 봇 시험이 running delta를 실제 open으로 착각해 clock을 너무 일찍 움직였다. 관측을 실제5/6/7 opened progress로 강화하여9개Green(0.16s)을 확인했다. 후속 리뷰는 같은 시각의 오류 정리가 UUID 정렬을 사용해 새 오류를 버리는 반례를 실행했다(Idle != Failed(Unavailable), .tmp/lobby58-history-red.log). 오류 map과 함께 bounded FIFO 기록 순서를 유지하도록 수정했다.

추가 공급 error/panic/2초 timeout/cancel·bounded collision8회·nil/limits/clock와 오류 FIFO 검사까지 service15개Green(2.03s), locked workspace all-target clippy exit0이다. 실제 blocking 준비 중 취소→peer 복귀/새 human 예약, room unready→새 generation,5초 준비/10분 방 경계·Drop, registry capacity 실패의 무점유와 새 membership의 이전 오류 우선 차단을 확인했다. 실패 기록은30초/capacity로 제한하며 새 성공 입력은 해당 기록을 지운다. 이전 cancel identity는 새 ticket을 제거할 수 없다.

검토 범위는 develop8dc226b 대비 private prepared state→bounded IO/CPU→keyed completion→live policy/registry→public assignment/snapshot이다. 공개 assignment는 match UUID·자기 seat·human/bot만 포함하고 다른 account·seed/Board를 보내지 않는다. registry의 조회 오류를 빈 match로 취급하지 않고 fail closed로 반환한다. core start 시간과 policy commit 시간은 같은 값이고 무거운 인증/await를 Lobby lock 안에서 하지 않는다. 최종 whole/core/domain coverage·전체 검사·병합은 실제 PR 결과에 기록한다.

이 내부 service는 아직 인증 HTTP/WS lobby·열거 admission·OAuth invite round trip·8언어 친구/빠른대전 화면·production main에 연결되지 않았다. 실제 TCP/TLS/DB 계약은 해당 후속 통합 출구와 기존 실제 테스트를 구분해 확인한다. 외부키42·miniPC26·사람27과 상위17은 이 port 성공만으로 완료하지 않는다.
