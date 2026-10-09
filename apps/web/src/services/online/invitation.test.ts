// @vitest-environment node
import { expect, test } from "vitest";
import { roomCode, roomInvitation } from "./invitation";
test("normalizes only eight public ASCII code characters and builds a same-origin locale-only invitation", () => {
  expect(roomCode("  abcd2345 ")).toBe("ABCD2345");
  for (const value of [
    "ABCD234",
    "ABCD23455",
    "ABCD2340",
    "ABCD23I5",
    "ABCD23O5",
    "ＡBCD2345",
    "abcd2345?token=x",
    "https://evil.test/ABCD2345",
  ])
    expect(roomCode(value)).toBeNull();
  expect(
    roomInvitation(
      "https://game.test/en/friends?csrf=private",
      "ko",
      "ABCD2345",
    ),
  ).toBe("https://game.test/ko/friends?code=ABCD2345");
  expect(() =>
    roomInvitation("https://game.test", "../../evil", "ABCD2345"),
  ).toThrow();
  expect(() => roomInvitation("https://game.test", "en", "bad")).toThrow();
});
