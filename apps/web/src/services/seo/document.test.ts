import { describe, expect, test } from "vitest";
import { LOCALES } from "../locale";
import { createI18n } from "../i18n";
import { documentMetadata, PUBLIC_DOCUMENTS, publicOrigin } from "./document";
import { applyDocumentHead } from "./head";
import {
  publicDocuments,
  staticContent,
} from "../../../build/public-documents";
const origin = "https://sweeper.example.test";
test("only explicitly configured public HTTPS origins become canonical authorities", () => {
  expect(publicOrigin(undefined)).toBeUndefined();
  expect(publicOrigin("")).toBeUndefined();
  expect(publicOrigin(`${origin}/`)).toBe(origin);
  for (const value of [
    "http://example.test",
    "https://u:p@example.test",
    "https://example.test:443",
    "https://example.test:8443",
    "https://example.test/path",
    "https://example.test/?a=b",
    "https://example.test/#main",
    "https://localhost",
    "https://sub.localhost",
    "https://127.0.0.1",
    "https://[::1]",
    "//example.test",
    "bad",
    "https://example.test.",
  ])
    expect(() => publicOrigin(value), value).toThrow();
});
describe("localized public document contract", () => {
  for (const locale of LOCALES)
    test(locale, async () => {
      const i18n = await createI18n(locale);
      for (const route of PUBLIC_DOCUMENTS.filter((route) =>
        ["/", "/rules", "/storage", "/learn"].includes(route),
      )) {
        const url = new URL(
          `/${locale}${route}`,
          "https://untrusted-request.test",
        );
        const value = documentMetadata(url, locale, i18n, `${origin}/`);
        expect(value.noindex).toBe(false);
        expect(value.canonical).toBe(`${origin}/${locale}${route}`);
        expect(value.alternates).toHaveLength(9);
        expect(
          value.alternates.find((alternate) => alternate.language === locale)
            ?.href,
        ).toBe(value.canonical);
        expect(
          value.alternates.find(
            (alternate) => alternate.language === "x-default",
          )?.href,
        ).toBe(`${origin}/en${route}`);
        expect(value.title).not.toMatch(/untrusted|undefined|\{\{/);
        expect(value.description).toBeTruthy();
        expect(documentMetadata(url, locale, i18n).canonical).toBeUndefined();
        const doc = new DOMParser().parseFromString(
          staticContent(route, locale, i18n),
          "text/html",
        );
        expect(doc.querySelectorAll("h1")).toHaveLength(1);
        expect(doc.querySelectorAll("nav a[lang]")).toHaveLength(8);
        expect(doc.body.textContent).not.toMatch(/\{\{/);
        if (route === "/storage")
          expect(doc.body.textContent).toContain(
            i18n.t(locale, "storage.cookies.description"),
          );
        if (route === "/rules")
          expect(doc.body.textContent).toContain(i18n.t(locale, "rules.flags"));
      }
    });
});
test("private, game, unrecognized and contaminated URLs never expose metadata URLs or request text", async () => {
  const i18n = await createI18n("en");
  for (const path of [
    "/",
    "/en/practice",
    "/en/tutorial",
    "/en/daily",
    "/en/daily/rankings",
    "/en/daily/official",
    "/en/queue",
    "/en/friends?code=private-value",
    "/en/results",
    "/en/me",
    "/en/me/history",
    "/en/me/account",
    "/en/me/privacy",
    "/en/settings",
    "/en/login",
    "/en/onboarding",
    "/en/403",
    "/en/404",
    "/en/unknown",
    "/en/?token=private-value",
    "/en/rules#private-value",
    "/en/storage?x=private-value",
    "/ko/rules",
  ]) {
    const value = documentMetadata(new URL(path, origin), "en", i18n, origin);
    expect(value.noindex, path).toBe(true);
    expect(value.canonical, path).toBeUndefined();
    expect(value.alternates, path).toHaveLength(0);
    expect(JSON.stringify(value), path).not.toContain("private-value");
  }
  expect(
    documentMetadata(new URL("/en/#main", origin), "en", i18n, origin)
      .canonical,
  ).toBe(`${origin}/en/`);
});
test("head ownership follows private/public/locale transitions and restores the original document on disposal", async () => {
  const i18n = await createI18n("en");
  await i18n.load("ko");
  document.title = "Original";
  document.documentElement.lang = "fr";
  const initial = document.createElement("meta");
  initial.setAttribute("data-liar-seo", "");
  initial.name = "robots";
  initial.content = "noindex, noarchive";
  document.head.append(initial);
  const unrelated = document.createElement("meta");
  unrelated.name = "unrelated";
  document.head.append(unrelated);
  for (const [path, locale, privatePage] of [
    ["/en/rules", "en", false],
    ["/en/me", "en", true],
    ["/ko/", "ko", false],
    ["/ko/404?token=private-value", "ko", true],
  ] as const) {
    const dispose = applyDocumentHead(
      document,
      documentMetadata(new URL(path, origin), locale, i18n, origin),
    );
    expect(
      document.head.querySelectorAll('link[rel="canonical"]'),
    ).toHaveLength(privatePage ? 0 : 1);
    expect(
      document.head.querySelectorAll('link[rel="alternate"]'),
    ).toHaveLength(privatePage ? 0 : 9);
    expect(document.head.querySelectorAll('meta[name="robots"]')).toHaveLength(
      privatePage ? 1 : 0,
    );
    expect(document.documentElement.lang).toBe(locale);
    expect(document.head.innerHTML).not.toContain("private-value");
    dispose();
    expect(document.title).toBe("Original");
    expect(document.documentElement.lang).toBe("fr");
    expect(initial.isConnected && unrelated.isConnected).toBe(true);
  }
  initial.remove();
  unrelated.remove();
});
test("static copy is escaped rather than interpreted as markup", async () => {
  const i18n = await createI18n("en");
  const doc = new DOMParser().parseFromString(
    staticContent("/", "en", {
      ...i18n,
      t: () => '<img src=x onerror="bad()">&',
    }),
    "text/html",
  );
  expect(doc.querySelector("img")).toBeNull();
  expect(doc.querySelector("h1")?.textContent).toContain("<img");
});
test("build emits exactly 56 documents but indexes only published public information, safe fallback and an allowlisted sitemap only when configured", async () => {
  for (const configured of [undefined, origin]) {
    const plugin = publicDocuments(configured);
    const emitted: Array<{ fileName: string; source: string }> = [];
    const bundle = {
      "index.html": {
        type: "asset",
        source:
          '<html lang="ko"><head><title>Liar Sweeper</title></head><body><div id="app"></div><script type="module" src="/assets/main.js"></script></body></html>',
      },
    };
    const hook = plugin.generateBundle;
    if (typeof hook !== "function") throw Error("Missing build hook");
    await Reflect.apply(
      hook,
      {
        emitFile: (asset: { fileName: string; source: string }) =>
          emitted.push(asset),
      },
      [{}, bundle, false],
    );
    expect(
      emitted.filter((asset) => asset.fileName.endsWith(".html")),
    ).toHaveLength(56);
    expect(bundle["index.html"].source).toContain(
      'content="noindex, noarchive"',
    );
    for (const asset of emitted.filter((asset) =>
      asset.fileName.endsWith(".html"),
    )) {
      const doc = new DOMParser().parseFromString(asset.source, "text/html");
      expect(doc.querySelector("h1")).not.toBeNull();
      const indexed =
        configured &&
        !/\/(?:privacy|terms|consent)\.html$/.test(asset.fileName);
      expect(doc.querySelectorAll('link[rel="canonical"]')).toHaveLength(
        indexed ? 1 : 0,
      );
      expect(doc.querySelectorAll('link[rel="alternate"]')).toHaveLength(
        indexed ? 9 : 0,
      );
      expect(
        doc.querySelector('script[type="module"]')?.getAttribute("src"),
      ).toBe("/assets/main.js");
      expect(doc.documentElement.lang).toBe(asset.fileName.split("/")[0]);
    }
    const robots = emitted.find(
      (asset) => asset.fileName === "robots.txt",
    )!.source;
    const sitemap = emitted.find(
      (asset) => asset.fileName === "sitemap.xml",
    )?.source;
    expect(robots).toContain(
      configured ? `Sitemap: ${origin}/sitemap.xml` : "Disallow: /",
    );
    if (configured) {
      const xml = new DOMParser().parseFromString(sitemap!, "application/xml");
      expect(xml.querySelector("parsererror")).toBeNull();
      const urls = [...xml.querySelectorAll("loc")].map(
        (node) => node.textContent,
      );
      expect(urls).toHaveLength(32);
      expect(new Set(urls).size).toBe(32);
      expect(urls).toContain(`${origin}/zh-CN/storage`);
      expect(urls).toContain(`${origin}/en/learn`);
      expect(urls).not.toContain(`${origin}/en/tutorial`);
      expect(sitemap).not.toMatch(
        /me\/|login|tutorial|friends|queue|daily|undefined/,
      );
    } else expect(sitemap).toBeUndefined();
  }
});

test("preview preserves the built authority and text without an origin in its current environment", async () => {
  const { mkdtemp, writeFile, rm } = await import("node:fs/promises");
  const { tmpdir } = await import("node:os");
  const { join } = await import("node:path");
  const dir = await mkdtemp(join(tmpdir(), "liar-seo-preview-"));
  try {
    await writeFile(
      join(dir, "index.html"),
      '<html><head><meta data-liar-seo name="robots" content="noindex, noarchive"></head><body></body></html>',
    );
    await writeFile(
      join(dir, "robots.txt"),
      `User-agent: *\nSitemap: ${origin}/sitemap.xml\n`,
    );
    const { mkdir } = await import("node:fs/promises");
    await mkdir(join(dir, "ko"));
    const baked = `<html lang="ko"><head><link data-liar-seo rel="canonical" href="${origin}/ko/rules"></head><body>Built copy</body></html>`;
    await writeFile(join(dir, "ko/rules.html"), baked);
    const plugin = publicDocuments(undefined);
    type Request = { url: string; method: string; headers: { accept: string } };
    type Response = {
      statusCode: number;
      destroyed: boolean;
      setHeader(key: string, value: string): void;
      end(body: string | undefined): void;
    };
    type Handler = (
      request: Request,
      response: Response,
      next: (error?: unknown) => void,
    ) => void;
    let handler: Handler;
    const hook = plugin.configurePreviewServer;
    if (typeof hook !== "function") throw Error("Missing preview hook");
    Reflect.apply(hook, {}, [
      {
        config: { root: dir, build: { outDir: "." } },
        middlewares: {
          use: (callback: Handler) => {
            handler = callback;
          },
        },
      },
    ]);
    const dispatch = (url: string, method = "GET") =>
      new Promise<{
        body: string | undefined;
        headers: Record<string, string>;
        status: number;
      }>((resolve, reject) => {
        const headers: Record<string, string> = {};
        const response = {
          statusCode: 200,
          destroyed: false,
          setHeader: (key: string, value: string) => {
            headers[key] = value;
          },
          end: (body: string | undefined) =>
            resolve({ body, headers, status: response.statusCode }),
        };
        handler(
          { url, method, headers: { accept: "text/html" } },
          response,
          (error: unknown) => reject(error ?? Error("Unexpected pass-through")),
        );
      });
    expect((await dispatch("/ko/rules")).body).toBe(baked);
    expect((await dispatch("/robots.txt")).body).toContain(origin);
    expect((await dispatch("/sitemap.xml")).status).toBe(404);
    const privatePage = await dispatch("/ko/rules?token=private-value");
    expect(privatePage.headers["x-robots-tag"]).toBe("noindex, noarchive");
    expect(privatePage.headers["cache-control"]).toBe("no-store");
    expect(privatePage.body).not.toContain(origin);
    expect(privatePage.body).not.toContain("private-value");
    expect((await dispatch("/ko/me", "HEAD")).body).toBeUndefined();
  } finally {
    await rm(dir, { recursive: true, force: true });
  }
});

test("unpublished policies and consent remain noindex with readable static fallback and escaped operator text", async () => {
  const i18n = await createI18n("ko");
  for (const route of ["/privacy", "/terms", "/consent"] as const) {
    const metadata = documentMetadata(
      new URL(`${origin}/ko${route}`),
      "ko",
      i18n,
      origin,
    );
    expect(metadata.noindex).toBe(true);
    expect(metadata.canonical).toBeUndefined();
    expect(staticContent(route, "ko", i18n)).toContain(
      route === "/consent"
        ? i18n.t("ko", "consent.pageIntro")
        : i18n.t("ko", "policy.pending"),
    );
  }
  const policy = {
    revision: "test",
    effectiveDate: "2026-11-14",
    operator: "<Operator>",
    contact: "test@example.test",
    privacy: [{ heading: "<script>", body: "<img src=x onerror=alert(1)>" }],
    terms: [{ heading: "Terms", body: "Test" }],
  };
  const doc = new DOMParser().parseFromString(
    staticContent("/privacy", "ko", i18n, policy),
    "text/html",
  );
  expect(doc.querySelector("script,img")).toBeNull();
  expect(doc.body.textContent).toContain("<Operator>");
  expect(
    documentMetadata(new URL(`${origin}/ko/privacy`), "ko", i18n, origin, true)
      .canonical,
  ).toBe(`${origin}/ko/privacy`);
});
test("configured public policies build escaped HTML and JSON while Google document markers stay isolated", async () => {
  const { mkdtemp, writeFile, rm } = await import("node:fs/promises");
  const { tmpdir } = await import("node:os");
  const { join } = await import("node:path");
  const folder = await mkdtemp(join(tmpdir(), "liar-policy-"));
  try {
    const policy = {
      revision: "synthetic",
      effectiveDate: "2026-11-14",
      operator: "<Operator>",
      contact: "test@example.test",
      privacy: [{ heading: "Privacy", body: "<script>text</script>" }],
      terms: [{ heading: "Terms", body: "Plain text" }],
    };
    const file = join(folder, "site.json");
    await writeFile(
      file,
      JSON.stringify({
        policies: Object.fromEntries(LOCALES.map((locale) => [locale, policy])),
        google: {
          frame: ["https://example.test"],
          connect: ["https://example.test"],
          image: ["https://example.test"],
        },
      }),
    );
    const plugin = publicDocuments(origin, file, "ca-pub-0000000000000001");
    const emitted: Array<{ fileName: string; source: string }> = [];
    const bundle = {
      "index.html": {
        type: "asset",
        source:
          '<html lang="en"><head><title>Liar Sweeper</title><meta name="referrer" content="no-referrer"></head><body><div id="app"></div><script type="module" src="/assets/main.js"></script></body></html>',
      },
    };
    if (typeof plugin.generateBundle !== "function")
      throw Error("Missing hook");
    await Reflect.apply(
      plugin.generateBundle,
      {
        emitFile: (asset: { fileName: string; source: string }) =>
          emitted.push(asset),
      },
      [{}, bundle, false],
    );
    expect(
      emitted.filter((asset) => asset.fileName.startsWith("policies/")),
    ).toHaveLength(8);
    expect(
      emitted.find((asset) => asset.fileName === "ads.txt")?.source,
    ).toContain("pub-0000000000000001");
    for (const locale of LOCALES) {
      const privacy = emitted.find(
        (asset) => asset.fileName === `${locale}/privacy.html`,
      )!.source;
      expect(privacy).toContain("&lt;Operator&gt;");
      expect(privacy).not.toContain('name="liar-google-document"');
      expect(privacy).not.toContain("<script>text</script>");
      expect(privacy).toContain(`href="${origin}/${locale}/privacy"`);
      const home = emitted.find(
        (asset) => asset.fileName === `${locale}/index.html`,
      )!.source;
      expect(home).toContain('name="liar-google-document" content="ready"');
      expect(home).toContain('name="referrer" content="strict-origin"');
      expect(home).toContain('nonce="LIAR_DOCUMENT_NONCE"');
    }
    expect(bundle["index.html"].source).not.toContain(
      'name="liar-google-document"',
    );
    expect(
      emitted
        .find((asset) => asset.fileName === "sitemap.xml")!
        .source.match(/<url>/g),
    ).toHaveLength(48);
  } finally {
    await rm(folder, { recursive: true, force: true });
  }
});
