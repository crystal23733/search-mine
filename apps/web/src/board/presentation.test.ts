// @vitest-environment node
import { expect, test } from "vitest";
import { coordinate, cellText, moveFocus } from "./presentation";
test("public cell text cannot expose a number on a closed or mined cell", () => {
  expect(
    cellText({ cell: 0, state: "closed", number: 7, flagged: false }),
  ).toBe("");
  expect(cellText({ cell: 0, state: "mine", number: 7, flagged: false })).toBe(
    "✹",
  );
  expect(cellText({ cell: 0, state: "safe", number: 0, flagged: false })).toBe(
    "",
  );
  expect(coordinate(26, 27)).toBe("AA1");
});
test("keyboard boundaries preserve rows and support first/last row and board cells", () => {
  expect(moveFocus(3, 4, 3, "ArrowRight", false)).toBe(3);
  expect(moveFocus(4, 4, 3, "ArrowLeft", false)).toBe(4);
  expect(moveFocus(0, 4, 3, "ArrowUp", false)).toBe(0);
  expect(moveFocus(11, 4, 3, "ArrowDown", false)).toBe(11);
  expect(moveFocus(5, 4, 3, "Home", false)).toBe(4);
  expect(moveFocus(5, 4, 3, "End", false)).toBe(7);
  expect(moveFocus(5, 4, 3, "Home", true)).toBe(0);
  expect(moveFocus(5, 4, 3, "End", true)).toBe(11);
  expect(moveFocus(5, 4, 3, "Tab", false)).toBe(5);
});
