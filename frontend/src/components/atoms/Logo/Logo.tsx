import { memo } from "react";
import { sizeClasses, themeClasses } from "./style/Logo.style";
import { cn } from "@/utils/cn";
import { Grid3X3, Zap } from "lucide-react";

/**
 * Logo 컴포넌트의 Props 인터페이스
 * @interface LogoProps
 */
export interface LogoProps {
  /**
   * 로고의 크기
   * - sm: 작은 크기 (높이 32px)
   * - md: 중간 크기 (높이 48px)
   * - lg: 큰 크기 (높이 64px)
   * - xl: 매우 큰 크기 (높이 80px)
   * @default "md"
   */
  size?: "sm" | "md" | "lg" | "xl";

  /**
   * 로고의 테마 색상
   * - primary: 기본 테마 색상
   * - white: 흰색 테마 색상
   * - dark: 어두운 테마 색상
   * - mono: 모노 테마 색상
   * @default "primary"
   */
  theme?: "primary" | "white" | "dark" | "mono";

  /**
   * 텍스트 로고 표시 여부
   * true: 아이콘과 텍스트 모두 표시
   * false: 아이콘만 표시
   * @default true
   */
  showText?: boolean;

  /**
   * 로고 클릭 시 호출되는 함수
   * 홈페이지 이동
   */
  onClick?: () => void;

  /**
   * 추가적인 CSS 클래스명
   */
  className?: string;

  /**
   * 접근성을 위한 aria-label
   * @default "Mines Battle Logo"
   */
  "aria-label"?: string;
}

const Logo = memo(
  ({
    size = "md",
    theme = "primary",
    showText = true,
    onClick,
    className = "",
    "aria-label": ariaLabel,
    ...rest
  }: LogoProps) => {
    const currentSize = sizeClasses[size];
    const currentTheme = themeClasses[theme];

    return (
      <div
        {...rest}
        className={cn(
          "flex items-center gap-3",
          currentSize.container,
          onClick &&
            "cursor-pointer hover:opacity-80 transition-opacity focus:outline-none focus:ring-2 focus:ring-blue-500 focus:ring-offset-2 rounded",
          className
        )}
        onClick={onClick}
        {...(onClick && {
          role: "button",
          tabIndex: 0,
          "aria-label": ariaLabel || "Mines Battle 홈으로 이동",
          onKeyDown: e => {
            if (e.key === "Enter" || e.key === " ") {
              e.preventDefault();
              onClick();
            }
          },
        })}
      >
        <div className="relative" aria-hidden="true">
          <Grid3X3 className={cn(currentSize.icon, currentTheme.icon, "opacity-60")} />
          <Zap
            className={cn(
              currentSize.icon,
              currentTheme.icon,
              "absolute top-1/2 left-1/2 transform -translate-x-1/2 -translate-y-1/2",
              size === "sm"
                ? "w-3 h-3"
                : size === "md"
                  ? "w-4 h-4"
                  : size === "lg"
                    ? "w-6 h-6"
                    : "w-8 h-8"
            )}
          />

          {showText && (
            <div className="flex flex-col">
              <span
                className={cn(
                  "font-bold leading-none tracking-tight",
                  currentSize.text,
                  currentTheme.text
                )}
              >
                Mines
              </span>
              <span
                className={cn(
                  "font-semibold leading-none tracking-wide opacity-75",
                  size === "sm"
                    ? "text-xs"
                    : size === "md"
                      ? "text-sm"
                      : size === "lg"
                        ? "text-base"
                        : "text-lg",
                  currentTheme.text
                )}
              >
                BATTLE
              </span>
            </div>
          )}
        </div>
      </div>
    );
  }
);

Logo.displayName = "Logo";
export default Logo;
