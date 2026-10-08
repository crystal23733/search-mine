import type { AuthAccount } from "@liar/protocol";
import { useState } from "preact/hooks";
import { useUi } from "../context";
import { useSnapshot } from "../useSnapshot";
import { NicknameField } from "../molecules/NicknameField";
import { Button } from "../atoms/Button";
export type NicknameAction = "save" | "tutorial" | "skip";
export function NicknameForm({
  account,
  onboarding = false,
  onSaved,
}: {
  account: AuthAccount;
  onboarding?: boolean;
  onSaved(account: AuthAccount, action: NicknameAction): void;
}) {
  const { t, services } = useUi(),
    state = useSnapshot(services.auth);
  const [value, setValue] = useState(account.nickname ?? ""),
    [rotation, setRotation] = useState(0);
  const names = [
    t("auth.suggestion1"),
    t("auth.suggestion2"),
    t("auth.suggestion3"),
  ];
  const disabled =
    state.working ||
    state.status !== "ready" ||
    state.account?.id !== account.id;
  const save = async (action: NicknameAction) => {
    const result = await services.auth.nickname(value, account.id);
    if (result.ok && services.auth.read().account?.id === account.id)
      onSaved(result.value, action);
  };
  return (
    <form
      class="nickname-form"
      onSubmit={(e) => {
        e.preventDefault();
        void save(onboarding ? "tutorial" : "save");
      }}
    >
      <NicknameField
        value={value}
        onInput={setValue}
        disabled={disabled}
        invalid={state.error === "auth_invalid"}
      />
      <h3>{t("auth.suggestions")}</h3>
      <div class="name-suggestions">
        {names.map((_, index) => {
          const name = names[(index + rotation) % names.length];
          return (
            <Button
              key={index}
              disabled={disabled}
              onClick={() => setValue(name)}
            >
              {name}
            </Button>
          );
        })}
        <Button
          disabled={disabled}
          variant="ghost"
          onClick={() => setRotation((rotation + 1) % names.length)}
        >
          {t("auth.moreNames")}
        </Button>
      </div>
      <p class="muted">{t("auth.nicknamePrivacy")}</p>
      <Button
        type="submit"
        variant="primary"
        disabled={disabled || !value.trim()}
      >
        {t(onboarding ? "auth.tutorial" : "auth.save")}
      </Button>
      {onboarding && (
        <Button
          variant="ghost"
          disabled={disabled || !value.trim()}
          onClick={() => {
            void save("skip");
          }}
        >
          {t("auth.skip")}
        </Button>
      )}
    </form>
  );
}
