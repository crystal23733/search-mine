import { publicOrigin } from "./seo/document.ts";
import { LOCALES, type Locale } from "./locale.ts";
export interface PolicySection {
  heading: string;
  body: string;
}
export interface PublicPolicy {
  revision: string;
  effectiveDate: string;
  operator: string;
  contact: string;
  privacy: PolicySection[];
  terms: PolicySection[];
}
export interface PublicSiteConfig {
  policies: Record<Locale, PublicPolicy>;
  google: { frame: string[]; connect: string[]; image: string[] };
}
const record = (value: unknown): Record<string, unknown> => {
  if (!value || typeof value !== "object" || Array.isArray(value))
    throw Error("Invalid public configuration");
  return value as Record<string, unknown>;
};
const text = (value: unknown, max = 12000) => {
  if (
    typeof value !== "string" ||
    !value.trim() ||
    value.length > max ||
    Array.from(value).some((character) => {
      const code = character.charCodeAt(0);
      return code < 32 && ![9, 10, 13].includes(code);
    })
  )
    throw Error("Invalid public text");
  return value;
};
const keys = (value: Record<string, unknown>, expected: string[]) => {
  if (
    Object.keys(value).length !== expected.length ||
    !expected.every((key) => Object.hasOwn(value, key))
  )
    throw Error("Unknown public configuration field");
};
export function decodePublicPolicy(value: unknown): PublicPolicy {
  if (new TextEncoder().encode(JSON.stringify(value)).length > 128 * 1024)
    throw Error("Policy exceeds 128 KiB");
  const v = record(value);
  keys(v, [
    "revision",
    "effectiveDate",
    "operator",
    "contact",
    "privacy",
    "terms",
  ]);
  const sections = (input: unknown) => {
    if (!Array.isArray(input) || !input.length || input.length > 30)
      throw Error("Invalid policy sections");
    return input.map((item) => {
      const s = record(item);
      keys(s, ["heading", "body"]);
      return { heading: text(s.heading, 200), body: text(s.body) };
    });
  };
  const effectiveDate = text(v.effectiveDate, 10);
  if (
    !/^\d{4}-\d{2}-\d{2}$/.test(effectiveDate) ||
    new Date(effectiveDate).toISOString().slice(0, 10) !== effectiveDate
  )
    throw Error("Invalid policy date");
  return {
    revision: text(v.revision, 100),
    effectiveDate,
    operator: text(v.operator, 300),
    contact: text(v.contact, 500),
    privacy: sections(v.privacy),
    terms: sections(v.terms),
  };
}
export function decodePublicSiteConfig(value: unknown): PublicSiteConfig {
  const v = record(value);
  keys(v, ["policies", "google"]);
  const policies = record(v.policies);
  keys(policies, [...LOCALES]);
  const google = record(v.google);
  keys(google, ["frame", "connect", "image"]);
  const origins = (value: unknown): string[] => {
    if (!Array.isArray(value) || value.length > 30)
      throw Error("Missing Google CSP origins");
    return value.map((raw) => {
      const s = text(raw, 200),
        url = new URL(s);
      if (
        url.protocol !== "https:" ||
        url.origin !== s ||
        publicOrigin(s) !== s ||
        url.port ||
        !url.hostname.includes(".") ||
        /[\s*'";$]/.test(s)
      )
        throw Error("Invalid CSP origin");
      return s;
    });
  };
  return {
    policies: Object.fromEntries(
      LOCALES.map((locale) => [locale, decodePublicPolicy(policies[locale])]),
    ) as Record<Locale, PublicPolicy>,
    google: {
      frame: origins(google.frame),
      connect: origins(google.connect),
      image: origins(google.image),
    },
  };
}
