import { describe, expect, it } from "vitest";
import { chooseLocale, normalizeLocale, localizedPath } from "./locale";
import { createPreferences } from "./preferences";
describe("locale and preference policy", () => {
  it("prioritizes a supported URL, then saved choice, then browser only at root", () => {
    expect(chooseLocale("/ko/settings", "fr", ["ja-JP"])).toBe("ko");
    expect(chooseLocale("/", "de", ["ko-KR"])).toBe("de");
    expect(chooseLocale("/", undefined, ["it-IT", "pt-PT"])).toBe("pt-BR");
    expect(chooseLocale("/room", "de", ["ko-KR"])).toBe("en");
    expect(chooseLocale("/", undefined, ["zh-TW"])).toBe("en");
    expect(normalizeLocale("zh-Hans")).toBe("zh-CN");
    expect(normalizeLocale("zh-Hant")).toBeUndefined();
    expect(localizedPath("/en/match/opaque-id", "ja")).toBe(
      "/ja/match/opaque-id",
    );
  });
  it("preserves in-memory preferences when storage writes fail and bounds malformed input", () => {
    const storage = {
      getItem: () => "{bad",
      setItem: () => {
        throw new Error("denied");
      },
    };
    const preferences = createPreferences(storage);
    let updates = 0;
    const stop = preferences.subscribe(() => updates++);
    preferences.update({
      locale: "ko",
      motion: "reduce",
      contrast: "high",
      zoom: 1.5,
    });
    expect(preferences.read()).toEqual({
      locale: "ko",
      motion: "reduce",
      contrast: "high",
      zoom: 1.5,
    });
    expect(updates).toBe(1);
    expect(preferences.persistent()).toBe(false);
    stop();
    preferences.update({ zoom: 2 });
    expect(updates).toBe(1);
    expect(
      createPreferences({
        getItem: () => "x".repeat(2049),
        setItem: () => {},
      }).read().zoom,
    ).toBe(1);
  });
  it("keeps storage available after invalid JSON and recovers from a later successful write", () => {
    let blocked = true;
    const preferences = createPreferences({
      getItem: () => "{bad",
      setItem: () => {
        if (blocked) throw new Error("denied");
      },
    });
    expect(preferences.persistent()).toBe(true);
    preferences.update({ zoom: 2 });
    expect(preferences.persistent()).toBe(false);
    blocked = false;
    preferences.update({ zoom: 1.25 });
    expect(preferences.persistent()).toBe(true);
  });
});
