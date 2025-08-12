// 크기별 스타일 정의
export const sizeClasses = {
  sm: { container: "h-8", icon: "w-8 h-8", text: "text-lg" },
  md: { container: "h-12", icon: "w-12 h-12", text: "text-2xl" },
  lg: { container: "h-16", icon: "w-16 h-16", text: "text-3xl" },
  xl: { container: "h-20", icon: "w-20 h-20", text: "text-4xl" },
};

// 테마별 색상 정의
export const themeClasses = {
  primary: { icon: "text-blue-500", text: "text-gray-900 dark:text-white" },
  white: { icon: "text-white", text: "text-white" },
  dark: { icon: "text-gray-800", text: "text-gray-900" },
  mono: { icon: "text-gray-600", text: "text-gray-700 dark:text-gray-300" },
};
