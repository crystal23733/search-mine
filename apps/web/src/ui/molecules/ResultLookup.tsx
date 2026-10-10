import type { OnlineState } from "../../services/online/controller";
import { useUi } from "../context";
import { Card } from "../atoms/Card";
import { Button } from "../atoms/Button";
export function ResultLookup({
  status,
  onCheck,
}: {
  status: OnlineState["resultLookup"];
  onCheck(): void;
}) {
  const { t } = useUi();
  return (
    <Card class="result-lookup">
      {status !== "idle" && (
        <p role={status === "loading" ? "status" : "alert"}>
          {t(
            status === "loading"
              ? "result.checking"
              : status === "not_found"
                ? "result.notFound"
                : "result.unavailable",
          )}
        </p>
      )}
      <Button disabled={status === "loading"} onClick={onCheck}>
        {t("result.check")}
      </Button>
    </Card>
  );
}
