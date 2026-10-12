import {
  fireEvent,
  render,
  screen,
  waitFor,
  within,
} from "@testing-library/preact";
import { expect, test, vi } from "vitest";
import { Ui } from "../ui/context";
import { PendingSubmission } from "../ui/organisms/PendingSubmission";
import { dailyTestPorts } from "../../test/daily-ports";
import { authTestPorts, TEST_ACCOUNT } from "../../test/auth-ports";
import { record } from "../../test/daily-record";
import { createI18n } from "../services/i18n";
import { createNavigation } from "../services/navigation";
import { createPreferences } from "../services/preferences";
import { createLearning } from "../services/learning";
import type { AppServices } from "../services/ports";
import type { SubmissionResult } from "../services/pending-submissions";
async function setup(authenticated = true) {
  window.history.replaceState(null, "", "/en/settings");
  const ports = authTestPorts(authenticated ? TEST_ACCOUNT : null, true);
  await ports.auth.refresh();
  const flush = vi.fn(async (): Promise<SubmissionResult> => ({
    sent: 1,
    removed: 1,
    retained: false,
  }));
  const services: AppServices = {
    ...dailyTestPorts(),
    ...ports,
    pendingSubmissions: { flush },
    i18n: await createI18n("en"),
    navigation: createNavigation(window),
    preferences: createPreferences(),
    learning: createLearning(),
    practiceCore: async () => {
      throw Error("unused");
    },
    trainingCore: async () => {
      throw Error("unused");
    },
    boardRenderer: async () => {
      throw Error("unused");
    },
  };
  await services.dailyRecords.saveFirst(record());
  render(
    <Ui.Provider
      value={{
        locale: "en",
        services,
        preferences: services.preferences.read(),
        selectLocale: () => {},
        t: (key, data) => services.i18n.t("en", key, data),
      }}
    >
      <PendingSubmission count={1} refresh={async () => {}} />
    </Ui.Provider>,
  );
  const button = await screen.findByRole("button", {
    name: "Submit pending results",
  });
  await waitFor(() =>
    expect(screen.getByText("Pending local results: 1")).toBeTruthy(),
  );
  if (authenticated)
    await waitFor(() =>
      expect((button as HTMLButtonElement).disabled).toBe(false),
    );
  return { ...ports, services, flush, button };
}
test("pending submission requires explicit current-account confirmation, close does not flush", async () => {
  const s = await setup();
  expect(s.flush).not.toHaveBeenCalled();
  fireEvent.click(s.button);
  const dialog = await screen.findByRole("dialog", {
    name: "Submit pending results",
  });
  expect(dialog.textContent).toContain(TEST_ACCOUNT.nickname);
  expect(s.flush).not.toHaveBeenCalled();
  fireEvent.click(within(dialog).getByRole("button", { name: "Close" }));
  expect(s.flush).not.toHaveBeenCalled();
  fireEvent.click(s.button);
  fireEvent.click(
    within(
      await screen.findByRole("dialog", { name: "Submit pending results" }),
    ).getByRole("button", { name: "Submit to this account" }),
  );
  await waitFor(() =>
    expect(s.flush).toHaveBeenCalledExactlyOnceWith(TEST_ACCOUNT.id),
  );
  await waitFor(() =>
    expect(screen.getByRole("status").textContent).toContain("1 submitted"),
  );
  expect(s.services.activity.read().busy).toBe(false);
});
test("guest cannot submit pending candidates", async () => {
  const s = await setup(false);
  expect((s.button as HTMLButtonElement).disabled).toBe(true);
  expect(s.flush).not.toHaveBeenCalled();
});
test("authority invalidation closes confirmation without sending a candidate", async () => {
  const s = await setup();
  fireEvent.click(s.button);
  expect(await screen.findByRole("dialog")).toBeTruthy();
  s.auth.invalidate();
  await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
  expect(s.flush).not.toHaveBeenCalled();
});
test("one in-flight submission holds activity, blocks duplicates, and reports retained results", async () => {
  const s = await setup();
  let finish!: (r: SubmissionResult) => void;
  s.flush.mockImplementation(
    () =>
      new Promise((r) => {
        finish = r;
      }),
  );
  fireEvent.click(s.button);
  fireEvent.click(
    within(await screen.findByRole("dialog")).getByRole("button", {
      name: "Submit to this account",
    }),
  );
  await waitFor(() => expect(s.services.activity.read().busy).toBe(true));
  expect((s.button as HTMLButtonElement).disabled).toBe(true);
  fireEvent.click(s.button);
  expect(s.flush).toHaveBeenCalledTimes(1);
  finish({ sent: 0, removed: 0, retained: true });
  await waitFor(() =>
    expect(screen.getByRole("status").textContent).toContain("remain pending"),
  );
  await waitFor(() => expect(s.services.activity.read().busy).toBe(false));
});
