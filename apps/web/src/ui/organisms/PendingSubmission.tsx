import { useEffect, useState } from "preact/hooks";
import { useUi } from "../context";
import { useSnapshot } from "../useSnapshot";
import { Button } from "../atoms/Button";
import { Dialog } from "../atoms/Dialog";
import type { SubmissionResult } from "../../services/pending-submissions";
export function PendingSubmission({
  count,
  refresh,
}: {
  count: number;
  refresh(): Promise<void>;
}) {
  const { t, services } = useUi();
  const auth = useSnapshot(services.auth),
    offline = useSnapshot(services.offline),
    activity = useSnapshot(services.activity);
  const revision = services.auth.revision();
  const [confirmation, setConfirmation] = useState<{
    id: string;
    revision: number;
    name: string;
  } | null>(null);
  const [working, setWorking] = useState(false);
  const [result, setResult] = useState<SubmissionResult | null>(null);
  useEffect(() => {
    setConfirmation(null);
    setResult(null);
  }, [auth.account?.id, revision]);
  const available =
    auth.status === "ready" &&
    !!auth.account?.nickname &&
    services.auth.connected() &&
    offline.online &&
    !activity.busy &&
    !activity.locked &&
    !working &&
    count > 0;
  const current =
    confirmation !== null &&
    auth.account?.id === confirmation.id &&
    revision === confirmation.revision;
  return (
    <>
      <p>{t("offline.pending", { count })}</p>
      <p class="muted">{t("offline.pendingHint")}</p>
      <Button
        disabled={!available}
        onClick={() => {
          const account = services.auth.read().account;
          if (!account?.nickname) return;
          setResult(null);
          setConfirmation({
            id: account.id,
            name: account.nickname,
            revision: services.auth.revision(),
          });
        }}
      >
        {t("offline.submit")}
      </Button>
      {result && (
        <output>
          {t(
            result.retained ? "offline.submitRetained" : "offline.submitDone",
            { sent: result.sent, removed: result.removed },
          )}
        </output>
      )}
      <Dialog
        open={confirmation !== null}
        focusClose
        title={t("offline.submit")}
        onClose={() => setConfirmation(null)}
      >
        <p>{t("offline.submitAccount", { name: confirmation?.name ?? "" })}</p>
        <Button
          disabled={!available || !current}
          onClick={() => {
            const target = confirmation;
            if (
              !target ||
              services.auth.account() !== target.id ||
              services.auth.revision() !== target.revision ||
              !services.auth.connected() ||
              !services.offline.read().online ||
              services.activity.read().busy ||
              services.activity.read().locked ||
              working
            )
              return;
            setWorking(true);
            setConfirmation(null);
            const release = services.activity.hold();
            void services.pendingSubmissions
              .flush(target.id)
              .then((value) => {
                if (
                  services.auth.account() === target.id &&
                  services.auth.revision() === target.revision
                )
                  setResult(value);
                return refresh();
              })
              .catch(() => {
                if (
                  services.auth.account() === target.id &&
                  services.auth.revision() === target.revision
                )
                  setResult({ sent: 0, removed: 0, retained: true });
              })
              .finally(() => {
                release();
                setWorking(false);
              });
          }}
        >
          {t("offline.submitConfirm")}
        </Button>
      </Dialog>
    </>
  );
}
