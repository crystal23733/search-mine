import { memo } from "react";
import { sizeClasses, themeClasses } from "./style/Logo.style";
import { cn } from "@/utils/cn";
import { Grid3X3, Zap } from "lucide-react";
import LogoProps from "./Logo.interface";

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
