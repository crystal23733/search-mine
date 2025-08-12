/**
 * Divider 컴포넌트의 스타일 클래스 정의 모듈
 * 모든 디자인 로직을 담당합니다.
 *
 * @module DividerStyles
 */

import DividerProps from "../Divider.interface";

/**
 * 방향에 따른 기본 레이아웃 클래스 반환
 *
 * @param orientation - 구분선 방향
 * @returns Tailwind CSS 클래스 문자열
 */
export const getOrientationClasses = (orientation: DividerProps["orientation"]): string => {
  const classes = {
    horizontal: "w-full flex items-center",
    vertical: "h-full flex flex-col justify-center min-h-[20px] max-h-[200px]", // 세로 구분선 높이 제한
  };
  return classes[orientation || "horizontal"];
};

/**
 * 여백 설정에 따른 반응형 클래스 반환
 *
 * @param spacing - 여백 크기
 * @param orientation - 구분선 방향
 * @returns Tailwind CSS 클래스 문자열
 */
export const getSpacingClasses = (
  spacing: DividerProps["spacing"],
  orientation: DividerProps["orientation"]
): string => {
  const isHorizontal = orientation === "horizontal";
  const classes = {
    none: isHorizontal ? "my-0" : "mx-0",
    sm: isHorizontal ? "my-1 sm:my-2" : "mx-1 sm:mx-2",
    md: isHorizontal ? "my-2 sm:my-4" : "mx-2 sm:mx-4",
    lg: isHorizontal ? "my-3 sm:my-6" : "mx-3 sm:mx-6",
    xl: isHorizontal ? "my-4 sm:my-8" : "mx-4 sm:mx-8",
  };
  return classes[spacing || "md"];
};

/**
 * 두께 설정에 따른 클래스 반환
 *
 * @param thickness - 구분선 두께
 * @param orientation - 구분선 방향
 * @returns Tailwind CSS 클래스 문자열
 */
export const getThicknessClasses = (
  thickness: DividerProps["thickness"],
  orientation: DividerProps["orientation"]
): string => {
  const isHorizontal = orientation === "horizontal";
  const classes = {
    thin: isHorizontal ? "h-px" : "w-px",
    medium: isHorizontal ? "h-0.5" : "w-0.5",
    thick: isHorizontal ? "h-1" : "w-1",
  };
  return classes[thickness || "thin"];
};

/**
 * 색상 테마에 따른 테두리 클래스 반환
 *
 * @param color - 색상 테마
 * @returns Tailwind CSS 클래스 문자열
 */
export const getColorClasses = (color: DividerProps["color"]): string => {
  const classes = {
    default: "border-gray-200 dark:border-gray-700",
    primary: "border-blue-500 dark:border-blue-400",
    secondary: "border-purple-500 dark:border-purple-400",
    muted: "border-gray-100 dark:border-gray-800",
  };
  return classes[color || "default"];
};

/**
 * variant에 따른 스타일 클래스 반환
 *
 * @param variant - 구분선 스타일
 * @returns Tailwind CSS 클래스 문자열
 */
export const getVariantClasses = (variant: DividerProps["variant"]): string => {
  const classes = {
    solid: "border-solid",
    dashed: "border-dashed",
    dotted: "border-dotted",
    gradient: "bg-gradient-to-r from-transparent via-gray-300 to-transparent dark:via-gray-600",
  };
  return classes[variant || "solid"];
};

/**
 * 색상에 따른 배경 클래스 반환 (그라디언트가 아닌 경우)
 *
 * @param color - 색상 테마
 * @returns Tailwind CSS 클래스 문자열
 */
export const getBackgroundClasses = (color: DividerProps["color"]): string => {
  const classes = {
    default: "bg-gray-200 dark:bg-gray-700",
    primary: "bg-blue-500 dark:bg-blue-400",
    secondary: "bg-purple-500 dark:bg-purple-400",
    muted: "bg-gray-100 dark:bg-gray-800",
  };
  return classes[color || "default"];
};

/**
 * 텍스트 색상 클래스 반환 (반응형 폰트 크기 포함)
 *
 * @param color - 색상 테마
 * @returns Tailwind CSS 클래스 문자열
 */
export const getTextColorClasses = (color: DividerProps["color"]): string => {
  const classes = {
    default: "text-gray-500 dark:text-gray-400",
    primary: "text-blue-600 dark:text-blue-400",
    secondary: "text-purple-600 dark:text-purple-400",
    muted: "text-gray-400 dark:text-gray-500",
  };
  return classes[color || "default"];
};

/**
 * 텍스트 위치에 따른 정렬 클래스 반환
 *
 * @param textPosition - 텍스트 위치
 * @returns Tailwind CSS 클래스 문자열
 */
export const getTextPositionClasses = (textPosition: DividerProps["textPosition"]): string => {
  const classes = {
    left: "justify-start",
    center: "justify-center",
    right: "justify-end",
  };
  return classes[textPosition || "center"];
};

/**
 * 반응형 텍스트 스타일 클래스 반환
 *
 * @returns Tailwind CSS 클래스 문자열
 */
export const getResponsiveTextClasses = (): string => {
  return "px-2 sm:px-3 text-xs sm:text-sm font-medium whitespace-nowrap truncate max-w-[200px] sm:max-w-none";
};
