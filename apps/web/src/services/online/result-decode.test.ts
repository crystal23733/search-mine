// @vitest-environment node
import { expect, test } from "vitest";
import { decodePersonalResult } from "./decode";
import { matchId, personalResult } from "../../../test/online-fixture";

test("accepts only the requested historical result without attaching current rules or reconstructing completion", () => {
  for (const completed of [true, false]) {
    const result = personalResult();
    result.result.completed = completed;
    expect(decodePersonalResult({ v: 1, result }, matchId)).toEqual(result);
  }
});
test("accepts explicit unknown details only for an incomplete server failure abort", () => {
  const result = {
    ...personalResult(),
    end_elapsed_ms: null,
    own: null,
    result: { reason: "server_failure", outcome: "abort", completed: false },
  };
  expect(decodePersonalResult({ v: 1, result }, matchId)).toEqual(result);
  for (const invalid of [
    { ...result, end_elapsed_ms: 0 },
    { ...result, own: personalResult().own },
    { ...result, end_elapsed_ms: undefined },
    { ...result, own: undefined },
    { ...result, result: { ...result.result, completed: true } },
    { ...result, result: { ...result.result, reason: "timeout" } },
    { ...result, result: { ...result.result, outcome: "draw" } },
    { ...result, seed: "hidden" },
  ])
    expect(() =>
      decodePersonalResult({ v: 1, result: invalid }, matchId),
    ).toThrow();
});
test("rejects extra fields at every depth, wrong match/version, unsafe and inconsistent statistics", () => {
  const source = { v: 1, result: personalResult() };
  const corrupt = [
    { ...source, seed: "hidden" },
    { ...source, v: 2 },
    { ...source, result: { ...source.result, board: [] } },
    {
      ...source,
      result: {
        ...source.result,
        match_id: "22222222-2222-4222-8222-222222222222",
      },
    },
    {
      ...source,
      result: {
        ...source.result,
        match_id: "00000000-0000-0000-0000-000000000000",
      },
    },
    { ...source, result: { ...source.result, rules_hash: "A".repeat(64) } },
    {
      ...source,
      result: { ...source.result, end_elapsed_ms: Number.MAX_SAFE_INTEGER + 1 },
    },
    {
      ...source,
      result: {
        ...source.result,
        result: { ...source.result.result, completed: "true" },
      },
    },
    {
      ...source,
      result: {
        ...source.result,
        own: { ...source.result.own, opened_safe: 257 },
      },
    },
    {
      ...source,
      result: { ...source.result, own: { ...source.result.own, mistakes: -1 } },
    },
    {
      ...source,
      result: {
        ...source.result,
        own: { ...source.result.own, accusation_attempts: 4097 },
      },
    },
    {
      ...source,
      result: {
        ...source.result,
        own: { ...source.result.own, correct_accusations: 4 },
      },
    },
    {
      ...source,
      result: { ...source.result, own: { ...source.result.own, opponent: 0 } },
    },
    {
      ...source,
      result: {
        ...source.result,
        result: { ...source.result.result, reason: "unknown" },
      },
    },
  ];
  for (const raw of corrupt)
    expect(() => decodePersonalResult(raw, matchId)).toThrow();
});
