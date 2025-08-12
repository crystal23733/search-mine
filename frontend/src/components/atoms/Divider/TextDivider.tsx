import { createElement, memo } from "react";
import { cn } from "@/utils/cn";
import DividerProps from "./Divider.interface";
import {
  getOrientationClasses,
  getSpacingClasses,
  getThicknessClasses,
  getVariantClasses,
  getBackgroundClasses,
  getColorClasses,
  getTextColorClasses,
  getResponsiveTextClasses,
} from "./style/Divider.style";

/**
 * 텍스트가 포함된 Divider 렌더링 컴포넌트
 *
 * 텍스트 위치, 반응형 처리, 접근성을 모두 고려한 구분선을 렌더링합니다.
 * 성능 최적화를 위해 memo로 래핑되었습니다.
 *
 * @component
 * @param props - TextDivider 컴포넌트 props (text 포함)
 * @returns 텍스트가 있는 구분선 JSX 요소
 */
export const TextDivider = memo((props: DividerProps & { text: string }) => {
  const {
    orientation = "horizontal",
    variant = "solid",
    thickness = "thin",
    text,
    textPosition = "center",
    color = "default",
    spacing = "md",
    className = "",
    as = "div",
    ...rest
  } = props;

  const isGradient = variant === "gradient";

  /**
   * 구분선의 시각적 클래스들을 조합
   * 텍스트 위치에 따라 좌/우 선의 표시 여부를 결정합니다.
   *
   * @param side - 구분선의 위치 ('left' | 'right')
   * @returns 조합된 CSS 클래스 문자열
   */
  const getLineClasses = (side: "left" | "right"): string => {
    const isHidden =
      (textPosition === "left" && side === "left") ||
      (textPosition === "right" && side === "right");

    return cn(
      "flex-1 transition-opacity duration-200", // 부드러운 전환 효과
      getThicknessClasses(thickness, orientation),
      isGradient ? getVariantClasses(variant) : getBackgroundClasses(color),
      !isGradient && getVariantClasses(variant),
      !isGradient && getColorClasses(color),
      isHidden && "hidden"
    );
  };

  return createElement(
    as,
    {
      ...rest,
      className: cn(
        getOrientationClasses(orientation),
        getSpacingClasses(spacing, orientation),
        className
      ),
      role: "separator",
      "aria-label": `구분선: ${text}`,
      "aria-orientation": orientation,
    },
    [
      // 왼쪽 선
      <div key="left-line" className={getLineClasses("left")} aria-hidden="true" />,

      // 텍스트 (반응형 + 접근성)
      <span
        key="text"
        className={cn(getResponsiveTextClasses(), getTextColorClasses(color))}
        title={text} // 긴 텍스트의 경우 툴팁으로 전체 내용 표시
        aria-label={`구분 텍스트: ${text}`}
      >
        {text}
      </span>,

      // 오른쪽 선
      <div key="right-line" className={getLineClasses("right")} aria-hidden="true" />,
    ]
  );
});

TextDivider.displayName = "TextDivider";
