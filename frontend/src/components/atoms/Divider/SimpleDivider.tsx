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
} from "./style/Divider.style";

/**
 * 일반 Divider 렌더링 컴포넌트
 *
 * 텍스트가 없는 구분선의 렌더링을 담당합니다.
 * 접근성과 성능 최적화가 적용되었습니다.
 *
 * @component
 * @param props - SimpleDivider 컴포넌트 props
 * @returns 일반 구분선 JSX 요소
 */
export const SimpleDivider = memo((props: DividerProps) => {
  const {
    orientation = "horizontal",
    variant = "solid",
    thickness = "thin",
    color = "default",
    spacing = "md",
    className = "",
    as = "div",
    ...rest
  } = props;

  const isGradient = variant === "gradient";

  return createElement(as, {
    ...rest,
    className: cn(
      getOrientationClasses(orientation),
      getSpacingClasses(spacing, orientation),
      getThicknessClasses(thickness, orientation),
      isGradient ? getVariantClasses(variant) : getBackgroundClasses(color),
      !isGradient && getVariantClasses(variant),
      !isGradient && getColorClasses(color),
      orientation === "vertical" && "self-stretch",
      "transition-colors duration-200", // 테마 변경 시 부드러운 전환
      className
    ),
    role: "separator",
    "aria-orientation": orientation,
    "aria-label": `${orientation === "horizontal" ? "수평" : "수직"} 구분선`,
  });
});

SimpleDivider.displayName = "SimpleDivider";
