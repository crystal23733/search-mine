import { useLayoutEffect, useRef } from "preact/hooks";
import { useUi } from "../ui/context";
import { Card } from "../ui/atoms/Card";
export function StatusPage({ status }: { status: 403 | 404 }) {
  const { t, locale } = useUi();
  const title = t(
    status === 403 ? "error.forbiddenTitle" : "error.notFoundTitle",
  );
  const heading = useRef<HTMLHeadingElement>(null);
  useLayoutEffect(() => {
    const oldTitle = document.title,
      previous = document.querySelector<HTMLMetaElement>('meta[name="robots"]'),
      content = previous?.getAttribute("content"),
      meta = previous ?? document.createElement("meta");
    document.title = `${status} · ${title} | Liar Sweeper`;
    meta.name = "robots";
    meta.content = "noindex, noarchive";
    if (!previous) document.head.append(meta);
    return () => {
      document.title = oldTitle;
      if (!previous) meta.remove();
      else if (content === null) meta.removeAttribute("content");
      else meta.content = content ?? "";
    };
  }, [status, title]);
  useLayoutEffect(() => {
    heading.current?.focus({ preventScroll: true });
  }, [status]);
  return (
    <div class="reading-page status-page">
      <Card>
        <p class="muted">{status}</p>
        <h1 ref={heading} tabIndex={-1}>
          {title}
        </h1>
        <p>
          {t(
            status === 403
              ? "error.forbiddenDescription"
              : "error.notFoundDescription",
          )}
        </p>
        <div class="match-controls">
          <a href={`/${locale}/`} class="button button--primary">
            {t("error.home")}
          </a>
          {status === 403 && (
            <a href={`/${locale}/me/account`} class="button">
              {t("error.account")}
            </a>
          )}
        </div>
      </Card>
    </div>
  );
}
