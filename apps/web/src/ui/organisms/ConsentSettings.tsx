import { useLayoutEffect, useId, useRef, useState } from "preact/hooks";
import { useUi } from "../context";
import { useSnapshot } from "../useSnapshot";
import { Card } from "../atoms/Card";
import { Button } from "../atoms/Button";

export function ConsentSettings() {
  const { services, t } = useUi();
  const consent = services.consent;
  const view = useSnapshot(consent);
  const title = useId();
  const request = useRef<AbortController | undefined>(undefined);
  const [failed, setFailed] = useState(false);
  useLayoutEffect(
    () => () => {
      request.current?.abort();
      request.current = undefined;
    },
    [consent],
  );
  return (
    <section aria-labelledby={title}>
      <h2 id={title}>{t("consent")}</h2>
      <Card>
        <p>{t("consent.description")}</p>
        <output>
          {t(view.working ? "consent.opening" : `consent.state.${view.status}`)}
        </output>
        <dl class="consent-purposes">
          {(["analytics", "advertising"] as const).map((purpose) => (
            <div key={purpose}>
              <dt>{t(`consent.${purpose}`)}</dt>
              <dd>
                {t(view[purpose] ? "consent.allowed" : "consent.blocked")}
              </dd>
            </div>
          ))}
        </dl>
        <div class="match-controls">
          <Button
            disabled={!view.canReopen || view.working}
            onClick={() => {
              if (request.current) return;
              const controller = new AbortController();
              request.current = controller;
              setFailed(false);
              void consent
                .reopen(controller.signal)
                .catch(() => false)
                .then((ok) => {
                  if (
                    request.current === controller &&
                    !controller.signal.aborted
                  )
                    setFailed(!ok);
                })
                .finally(() => {
                  if (request.current === controller)
                    request.current = undefined;
                });
            }}
          >
            {t("consent.reopen")}
          </Button>
          {view.working && (
            <Button variant="ghost" onClick={() => request.current?.abort()}>
              {t("consent.cancel")}
            </Button>
          )}
        </div>
        {failed && <p role="alert">{t("consent.reopenFailed")}</p>}
      </Card>
    </section>
  );
}
