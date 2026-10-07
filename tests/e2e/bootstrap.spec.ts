import { expect, test } from "@playwright/test";

test("web entry boots on desktop and mobile without collecting credentials", async ({
  page,
}) => {
  await page.goto("/");
  await expect(page.getByRole("link", { name: "Liar Sweeper" })).toBeVisible();
  await expect(page.getByRole("heading", { level: 1 })).toHaveCount(1);
  await expect(page.locator("input[type=password]")).toHaveCount(0);
  await expect(page.locator("body")).not.toBeEmpty();
  expect(
    await page.evaluate(
      () => document.documentElement.scrollWidth <= innerWidth,
    ),
  ).toBe(true);
});
