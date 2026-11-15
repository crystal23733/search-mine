import { useEffect, useState } from "preact/hooks";
import { useUi } from "../ui/context";
import {
  decodePublicPolicy,
  type PublicPolicy,
} from "../services/public-policy";
export function PolicyPage({ kind }: { kind: "privacy" | "terms" }) {
  const { locale, t } = useUi();
  const [value, setValue] = useState<
    { locale: string; policy: PublicPolicy } | undefined
  >();
  useEffect(() => {
    const controller = new AbortController();
    void fetch(`/policies/${locale}.json`, {
      signal: controller.signal,
      credentials: "omit",
      redirect: "error",
    })
      .then(async (response) => {
        if (
          !response.ok ||
          Number(response.headers.get("content-length")) > 256 * 1024
        )
          throw Error("unavailable");
        const body = await response.text();
        if (body.length > 256 * 1024) throw Error("unavailable");
        return decodePublicPolicy(JSON.parse(body));
      })
      .then((policy) => {
        if (!controller.signal.aborted) setValue({ locale, policy });
      })
      .catch(() => {});
    return () => controller.abort();
  }, [locale]);
  const policy = value?.locale === locale ? value.policy : undefined;
  return (
    <article class="reading-page">
      <h1>{t(kind)}</h1>
      {policy ? (
        <>
          <p>
            {policy.operator} · {policy.contact}
          </p>
          <p>
            {policy.effectiveDate} · {policy.revision}
          </p>
          {policy[kind].map((section) => (
            <section key={section.heading}>
              <h2>{section.heading}</h2>
              <p style={{ whiteSpace: "pre-line" }}>{section.body}</p>
            </section>
          ))}
        </>
      ) : (
        <output>{t("policy.pending")}</output>
      )}
    </article>
  );
}
