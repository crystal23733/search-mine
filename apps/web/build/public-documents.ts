import type { PublicPolicy } from "../src/services/public-policy.ts";
import { readSiteConfig } from "./site-config.ts";
import type { Plugin, ViteDevServer } from "vite";
import { readFile } from "node:fs/promises";
import { resolve } from "node:path";
import { createTranslation } from "../src/services/i18n-engine.ts";
import { GUIDE_STEPS } from "../src/services/tutorial-guide.ts";
import { ruleCopyData } from "../src/services/rule-copy.ts";
import { type I18nPort, type MessageKey } from "../src/services/i18n-engine.ts";
import {
  LOCALES,
  LOCALE_NAMES,
  isLocale,
  type Locale,
} from "../src/services/locale.ts";
import {
  documentStatus,
  documentRoute,
  isApplicationDocument,
} from "../src/services/document-route.ts";
import {
  documentMetadata,
  PUBLIC_DOCUMENTS,
  publicOrigin,
  publicPath,
  localizedDocument,
  type PublicDocument,
} from "../src/services/seo/document.ts";
export const escapeHtml = (value: string) =>
  value.replace(
    /[&<>"']/g,
    (character) =>
      ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" })[
        character
      ]!,
  );
export function staticContent(
  route: PublicDocument,
  locale: Locale,
  i18n: I18nPort,
  policy?: PublicPolicy,
): string {
  const t = (key: MessageKey, data?: Record<string, string | number>) =>
    escapeHtml(i18n.t(locale, key, data));
  const link = (path: string, text: string, className = "") =>
    `<a href="/${locale}${path}" class="${className}">${text}</a>`;
  const basics = () =>
    `<p>${t("basics.goal")}</p><dl>${(["numbers", "flags", "controls", "lies"] as const).map((topic) => `<dt>${t(`basics.${topic}.title`)}</dt><dd>${t(`basics.${topic}`, { open: i18n.t(locale, "board.open"), flag: i18n.t(locale, "board.flag"), accuse: i18n.t(locale, "board.accuse") })}</dd>`).join("")}</dl>`;
  let title: string, body: string;
  if (route === "/privacy" || route === "/terms") {
    title = t(route.slice(1) as "privacy" | "terms");
    body = policy
      ? `<p>${escapeHtml(policy.operator)} · ${escapeHtml(policy.contact)}</p><p>${escapeHtml(policy.effectiveDate)} · ${escapeHtml(policy.revision)}</p>${policy[route.slice(1) as "privacy" | "terms"].map((section) => `<section><h2>${escapeHtml(section.heading)}</h2><p style="white-space:pre-line">${escapeHtml(section.body)}</p></section>`).join("")}`
      : `<p>${t("policy.pending")}</p>`;
  } else if (route === "/consent") {
    title = t("consent.pageTitle");
    body = `<p>${t("consent.pageIntro")}</p><p>${t("consent.accountOptional")}</p><p>${link("/privacy", t("privacy"))}</p>`;
  } else if (route === "/") {
    title = `${t("home.title1")} ${t("home.title2")}`;
    body = `<p>${t("home.description")}</p><p>${link("/practice", t("home.practice"))}</p><p>${link("/rules", t("home.learn"))}</p>`;
  } else if (route === "/learn") {
    title = t("learn.title");
    body = `<p>${t("learn.description")}</p><h2>${t("learn.steps")}</h2><ol>${GUIDE_STEPS.map((step) => `<li><h3>${t(`home.${step}`)}</h3><p>${t(`home.${step}Hint`)}</p></li>`).join("")}</ol>${basics()}<p>${t("learn.ready")}</p><div class="match-controls">${link("/tutorial", t("learn.start"), "button button--primary")}${link("/rules", t("rules.title"), "button")}</div>`;
  } else if (route === "/rules") {
    title = t("rules.title");
    const data = ruleCopyData(locale);
    body =
      basics() +
      (
        [
          "rules.win",
          "rules.gauge",
          "rules.lies",
          "rules.stun",
          "rules.flags",
        ] as const
      )
        .map((key) => `<p>${t(key, data)}</p>`)
        .join("") +
      link("/tutorial", t("tutorial.replay"));
  } else {
    title = t("storage.pageTitle");
    body = `<p>${t("storage.intro")}</p><dl>${(["preferences", "records", "cache", "cookies", "optional"] as const).map((category) => `<dt>${t(`storage.${category}.title`)}</dt><dd>${t(`storage.${category}.description`)}</dd>`).join("")}</dl>${(["storage.manage", "storage.siteData", "storage.fallback"] as const).map((key) => `<p>${t(key)}</p>`).join("")}`;
  }
  return `<div class="app-shell"><a class="skip-link" href="#main">${t("skip")}</a><main id="main" class="reading-page" tabindex="-1"><h1>${title}</h1>${body}<p>${link("/", t("home"))} · ${link("/storage", t("storage.pageTitle"))} · ${link("/learn", t("learn.title"))}</p><nav aria-label="${t("language")}">${LOCALES.map((language) => `<a href="${localizedDocument(language, route)}" lang="${language}">${LOCALE_NAMES[language]}</a>`).join(" · ")}</nav></main></div>`;
}
export function publicDocuments(
  originValue: string | undefined,
  configPath?: string,
  publisher?: string,
): Plugin {
  const site = readSiteConfig(configPath);
  const origin = publicOrigin(originValue);
  let root = process.cwd();
  let pending: Promise<I18nPort> | undefined;
  const ready = () =>
    (pending ??= (async () => {
      const readMessages = async (locale: Locale) =>
        JSON.parse(
          await readFile(
            resolve(root, `src/locales/${locale}/common.json`),
            "utf8",
          ),
        ) as Record<MessageKey, string>;
      const messages = await createTranslation(
        await readMessages("en"),
        readMessages,
      );
      await Promise.all(LOCALES.map((locale) => messages.load(locale)));
      return messages;
    })());
  const robots = () =>
    origin
      ? `User-agent: *\nAllow: /\nSitemap: ${origin}/sitemap.xml\n`
      : "User-agent: *\nDisallow: /\n";
  const discoverable = PUBLIC_DOCUMENTS.filter(
    (route) =>
      route !== "/consent" && (site || !["/privacy", "/terms"].includes(route)),
  );
  const sitemap = () =>
    `<?xml version="1.0" encoding="UTF-8"?><urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">${LOCALES.flatMap((locale) => discoverable.map((route) => `<url><loc>${escapeHtml(`${origin}${localizedDocument(locale, route)}`)}</loc></url>`)).join("")}</urlset>`;
  async function html(
    source: string,
    url: URL,
  ): Promise<{ html: string; private: boolean }> {
    const locale = isLocale(url.pathname.split("/")[1])
      ? (url.pathname.split("/")[1] as Locale)
      : "en";
    const messages = await ready();
    const value = documentMetadata(
      url,
      locale,
      messages,
      origin,
      Boolean(site),
    );
    const meta = (attributes: string) => `<meta data-liar-seo ${attributes}>`;
    const tags = [
      value.noindex ? meta('name="robots" content="noindex, noarchive"') : "",
      value.description
        ? meta(`name="description" content="${escapeHtml(value.description)}"`)
        : "",
    ];
    if (value.canonical) {
      tags.push(
        `<link data-liar-seo rel="canonical" href="${escapeHtml(value.canonical)}">`,
      );
      tags.push(
        ...value.alternates.map(
          (alternate) =>
            `<link data-liar-seo rel="alternate" hreflang="${alternate.language}" href="${escapeHtml(alternate.href)}">`,
        ),
      );
      tags.push(
        meta('property="og:type" content="website"'),
        meta(`property="og:title" content="${escapeHtml(value.title)}"`),
        meta(
          `property="og:description" content="${escapeHtml(value.description!)}"`,
        ),
        meta(`property="og:url" content="${escapeHtml(value.canonical)}"`),
      );
    }
    // The configured entry is the only file read; request text is never an HTML template or a filesystem path.
    const clean = source.replace(
      /<(?:meta|link)\b[^>]*\bdata-liar-seo\b[^>]*>/g,
      "",
    );
    const route = publicPath(documentRoute(url.pathname));
    const publicGoogle =
      Boolean(publisher) &&
      ["/", "/rules", "/consent"].includes(route ?? "") &&
      !url.search;
    if (publicGoogle)
      tags.push('<meta name="liar-google-document" content="ready">');
    const template = publicGoogle
      ? clean.replace(
          'name="referrer" content="no-referrer"',
          'name="referrer" content="strict-origin"',
        )
      : clean;
    const content =
      route && !url.search && isLocale(url.pathname.split("/")[1])
        ? staticContent(route, locale, messages, site?.policies[locale])
        : "";
    return {
      private: value.noindex,
      html: template
        .replace(/<html\b[^>]*>/, `<html lang="${locale}">`)
        .replace(
          /<title>[^<]*<\/title>/,
          `<title>${escapeHtml(value.title)}</title>`,
        )
        .replace("</head>", `${tags.join("")}\n</head>`)
        .replace('<div id="app"></div>', `<div id="app">${content}</div>`),
    };
  }
  function installDev(server: ViteDevServer) {
    server.middlewares.use((request, response, next) => {
      if (!["GET", "HEAD"].includes(request.method ?? "GET")) return next();
      const url = new URL(request.url ?? "/", "http://local.invalid");
      if (url.pathname === "/robots.txt" || url.pathname === "/sitemap.xml") {
        if (url.pathname === "/sitemap.xml" && !origin) {
          response.statusCode = 404;
          response.end();
          return;
        }
        response.setHeader(
          "content-type",
          url.pathname === "/robots.txt"
            ? "text/plain; charset=utf-8"
            : "application/xml; charset=utf-8",
        );
        response.end(
          request.method === "HEAD"
            ? undefined
            : url.pathname === "/robots.txt"
              ? robots()
              : sitemap(),
        );
        return;
      }
      if (
        !request.headers.accept
          ?.split(",")
          .some(
            (type) => type.trim().split(";")[0].toLowerCase() === "text/html",
          ) ||
        !isApplicationDocument(url.pathname) ||
        documentStatus(url.pathname) !== 200
      )
        return next();
      void (async () => {
        let source = await readFile(
          resolve(server.config.root, "index.html"),
          "utf8",
        );
        source = await server.transformIndexHtml("/index.html", source);
        const value = await html(source, url);
        if (response.destroyed) return;
        response.setHeader("content-type", "text/html; charset=utf-8");
        response.setHeader("x-content-type-options", "nosniff");
        if (value.private) {
          response.setHeader("cache-control", "no-store");
          response.setHeader("x-robots-tag", "noindex, noarchive");
        }
        response.end(request.method === "HEAD" ? undefined : value.html);
      })().catch(next);
    });
  }
  return {
    name: "liar-public-documents",
    configResolved(config) {
      root = config.root;
    },
    enforce: "post",
    configureServer(server) {
      installDev(server);
    },
    configurePreviewServer(server) {
      // Preview serves immutable build artifacts, including their baked origin and translations.
      server.middlewares.use((request, response, next) => {
        if (!["GET", "HEAD"].includes(request.method ?? "GET")) return next();
        const url = new URL(request.url ?? "/", "http://local.invalid");
        const discovery =
          url.pathname === "/robots.txt" || url.pathname === "/sitemap.xml";
        if (
          !discovery &&
          (!request.headers.accept
            ?.split(",")
            .some(
              (type) => type.trim().split(";")[0].toLowerCase() === "text/html",
            ) ||
            !isApplicationDocument(url.pathname) ||
            documentStatus(url.pathname) !== 200)
        )
          return next();
        const language = url.pathname.split("/")[1];
        const route = publicPath(documentRoute(url.pathname));
        const readable =
          !discovery &&
          isLocale(language) &&
          route !== undefined &&
          !url.search;
        const file = discovery
          ? url.pathname.slice(1)
          : readable
            ? route === "/"
              ? `${language}/index.html`
              : `${language}${route}.html`
            : "index.html";
        void (async () => {
          let content: string;
          try {
            content = await readFile(
              resolve(server.config.root, server.config.build.outDir, file),
              "utf8",
            );
          } catch (error) {
            if (
              discovery &&
              (error as NodeJS.ErrnoException).code === "ENOENT"
            ) {
              response.statusCode = 404;
              response.end();
              return;
            }
            throw error;
          }
          if (response.destroyed) return;
          response.setHeader(
            "content-type",
            discovery
              ? url.pathname === "/robots.txt"
                ? "text/plain; charset=utf-8"
                : "application/xml; charset=utf-8"
              : "text/html; charset=utf-8",
          );
          response.setHeader("x-content-type-options", "nosniff");
          if (!discovery && !readable) {
            response.setHeader("cache-control", "no-store");
            response.setHeader("x-robots-tag", "noindex, noarchive");
          }
          response.end(request.method === "HEAD" ? undefined : content);
        })().catch(next);
      });
    },
    async generateBundle(_options, bundle) {
      const entry = bundle["index.html"];
      if (entry?.type !== "asset") throw new Error("Missing built HTML entry");
      const source = String(entry.source)
        .replace(/<script\b/g, '<script nonce="LIAR_DOCUMENT_NONCE"')
        .replace(
          /<link\b(?=[^>]*rel="modulepreload")/g,
          '<link nonce="LIAR_DOCUMENT_NONCE"',
        );
      if (publisher) {
        if (!site || !/^ca-pub-\d{16}$/.test(publisher ?? ""))
          throw Error("Google publisher configuration incomplete");
        this.emitFile({
          type: "asset",
          fileName: "ads.txt",
          source: `google.com, ${publisher!.slice(3)}, DIRECT, f08c47fec0942fa0\n`,
        });
      }
      if (site)
        for (const locale of LOCALES)
          this.emitFile({
            type: "asset",
            fileName: `policies/${locale}.json`,
            source: JSON.stringify(site.policies[locale]),
          });
      for (const locale of LOCALES)
        for (const route of PUBLIC_DOCUMENTS) {
          this.emitFile({
            type: "asset",
            fileName:
              route === "/" ? `${locale}/index.html` : `${locale}${route}.html`,
            source: (
              await html(
                source,
                new URL(
                  localizedDocument(locale, route),
                  "http://local.invalid",
                ),
              )
            ).html,
          });
        }
      entry.source = source.replace(
        "</head>",
        '<meta data-liar-seo name="robots" content="noindex, noarchive"></head>',
      );
      this.emitFile({
        type: "asset",
        fileName: "robots.txt",
        source: robots(),
      });
      if (origin)
        this.emitFile({
          type: "asset",
          fileName: "sitemap.xml",
          source: sitemap(),
        });
    },
  };
}
