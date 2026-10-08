import { useId } from "preact/hooks";
import { useUi } from "../context";
export function NicknameField({
  value,
  onInput,
  disabled,
  invalid,
}: {
  value: string;
  onInput(value: string): void;
  disabled: boolean;
  invalid: boolean;
}) {
  const { t, locale } = useUi(),
    id = useId(),
    hint = `${id}-hint`;
  const count =
    typeof Intl.Segmenter === "function"
      ? Array.from(
          new Intl.Segmenter(locale, { granularity: "grapheme" }).segment(
            value,
          ),
        ).length
      : Array.from(value).length;
  return (
    <div class="nickname-field">
      <label for={id}>{t("auth.nickname")}</label>
      <input
        id={id}
        type="text"
        name="nickname"
        autoComplete="off"
        spellcheck={false}
        maxLength={256}
        value={value}
        disabled={disabled}
        aria-invalid={invalid}
        aria-describedby={hint}
        onInput={(e) => onInput(e.currentTarget.value)}
      />
      <div id={hint} class="field-hint">
        <span>{t("auth.nicknameHint")}</span>
        <span>{t("auth.nicknameCount", { count })}</span>
      </div>
    </div>
  );
}
