import {
  act,
  fireEvent,
  render,
  screen,
  waitFor,
  within,
} from "@testing-library/preact";
import { expect, test, vi } from "vitest";
import type { DailyRankingsResponse } from "@liar/protocol";
import { dailyTestPorts } from "../../test/daily-ports";
import { record } from "../../test/daily-record";
import { createI18n } from "../services/i18n";
import { createNavigation } from "../services/navigation";
import { createPreferences } from "../services/preferences";
import { createLearning } from "../services/learning";
import { Ui } from "../ui/context";
import { DailyRankingsPage } from "./DailyRankingsPage";
import type { AppServices } from "../services/ports";
const empty: DailyRankingsResponse = {
  v: 1,
  metadata: null,
  time: { entries: [], has_more: false },
  completion: { entries: [], has_more: false },
};
async function setup(result = empty) {
  const ranking = vi.fn().mockResolvedValue(result),
    core = vi.fn();
  const base = dailyTestPorts();
  const services: AppServices = {
    ...base,
    dailyRankings: ranking,
    dailyCore: core,
    i18n: await createI18n("en"),
    navigation: createNavigation(window),
    preferences: createPreferences(),
    learning: createLearning(),
    practiceCore: core,
    trainingCore: core,
    boardRenderer: core,
  };
  const rendered = render(
    <Ui.Provider
      value={{
        locale: "en",
        services,
        preferences: services.preferences.read(),
        selectLocale: () => {},
        t: (key, data) => services.i18n.t("en", key, data),
      }}
    >
      <DailyRankingsPage />
    </Ui.Provider>,
  );
  const refresh = screen.getByRole("button", { name: "Refresh rankings" });
  await waitFor(() => expect(ranking).toHaveBeenCalledTimes(1));
  await waitFor(() =>
    expect((refresh as HTMLButtonElement).disabled).toBe(false),
  );
  return { ranking, core, services, refresh, ...rendered };
}
test("public unpublished, published empty and failed requests remain distinct and retryable", async () => {
  const s = await setup();
  expect(
    await screen.findByText("No puzzle has been published for this UTC date."),
  ).toBeTruthy();
  expect(screen.queryAllByRole("table")).toHaveLength(0);
  expect(s.core).not.toHaveBeenCalled();
  s.ranking.mockResolvedValue({ ...empty, metadata: record().metadata });
  fireEvent.click(s.refresh);
  expect(
    await screen.findByRole("table", { name: "Server time ranking" }),
  ).toBeTruthy();
  expect(
    screen.getAllByText("No verified completions in this list yet."),
  ).toHaveLength(2);
  s.ranking.mockRejectedValueOnce(Error("503"));
  fireEvent.click(s.refresh);
  expect((await screen.findByRole("alert")).textContent).toBe(
    "Rankings are unavailable. Try again.",
  );
  expect(screen.queryAllByRole("table")).toHaveLength(0);
  fireEvent.click(s.refresh);
  expect(
    await screen.findByRole("table", { name: "Verified completion ranking" }),
  ).toBeTruthy();
  s.unmount();
});
test("two accessible lists keep server positions and millisecond precision without identifying same nicknames", async () => {
  const result: DailyRankingsResponse = {
    ...empty,
    metadata: record().metadata,
    time: {
      entries: [
        { position: 1, nickname: "Same", mistakes: 2, elapsed_ms: 1234 },
        { position: 2, nickname: "Same", mistakes: 0, elapsed_ms: 1235 },
      ],
      has_more: false,
    },
    completion: {
      entries: Array.from({ length: 50 }, (_, i) => ({
        position: i + 1,
        nickname: "Same",
        mistakes: 0,
      })),
      has_more: true,
    },
  };
  const s = await setup(result);
  const time = await screen.findByRole("table", {
    name: "Server time ranking",
  });
  expect(within(time).getAllByRole("columnheader")).toHaveLength(4);
  expect(within(time).getByRole("cell", { name: "0:01.234" })).toBeTruthy();
  expect(within(time).getByRole("cell", { name: "0:01.235" })).toBeTruthy();
  const completion = screen.getByRole("table", {
    name: "Verified completion ranking",
  });
  expect(within(completion).getAllByRole("row")).toHaveLength(51);
  expect(within(completion).getAllByRole("columnheader")).toHaveLength(3);
  expect(
    screen.getByText(
      "Showing the first 50 entries. More verified completions exist.",
    ),
  ).toBeTruthy();
  expect(screen.queryByText(/\(me\)|completion_id|seed|rules_hash/)).toBeNull();
  s.unmount();
});
test("loading blocks duplicate refresh and unmount cancels its pending read", async () => {
  const s = await setup();
  let finish!: (value: DailyRankingsResponse) => void;
  s.ranking.mockImplementationOnce(
    () =>
      new Promise((resolve) => {
        finish = resolve;
      }),
  );
  fireEvent.click(s.refresh);
  expect(screen.getByRole("status").textContent).toBe("Loading rankings…");
  expect((s.refresh as HTMLButtonElement).disabled).toBe(true);
  expect(screen.queryAllByRole("table")).toHaveLength(0);
  const signal: AbortSignal = s.ranking.mock.calls.at(-1)![2];
  s.unmount();
  expect(signal.aborted).toBe(true);
  await act(async () => finish(empty));
});
