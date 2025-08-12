import { cn } from "@/utils/cn";
import { forwardRef } from "react";
import InputProps from "./Input.interface";

/**
 * 재사용 가능한 커스텀 Input 컴포넌트
 *
 * TailwindCSS 기반으로 구축된 고도로 커스터마이징 가능한 입력 필드입니다.
 * 라벨, 에러 메시지, 헬퍼 텍스트, 아이콘 등을 지원하며 접근성을 준수합니다.
 *
 * @component
 * @example
 * // 기본 사용법
 * <Input
 *   label="사용자명"
 *   placeholder="닉네임을 입력해주세요"
 *   helperText="3-20자 이내로 입력해주세요"
 * />
 *
 * @example
 * // 에러 상태
 * <Input
 *   label="이메일"
 *   error="올바른 이메일 주소를 입력해주세요"
 *   variant="outlined"
 * />
 *
 * @example
 * // 아이콘과 함께 사용
 * <Input
 *   label="검색"
 *   leftIcon={<SearchIcon />}
 *   rightIcon={<ClearButton />}
 *   placeholder="게임을 검색해보세요"
 * />
 *
 * @param {InputProps} props - Input 컴포넌트의 props
 * @param {React.Ref<HTMLInputElement>} ref - 입력 요소에 대한 ref
 * @returns {React.ForwardRefExoticComponent} 렌더링된 Input 컴포넌트
 */
const Input = forwardRef<HTMLInputElement, InputProps>(
  (
    {
      label,
      error,
      helperText,
      variant = "default",
      inputSize = "md",
      leftIcon,
      rightIcon,
      className = "",
      id,
      ...props
    },
    ref
  ) => {
    // 고유한 ID 생성 (접근성을 위해 라벨과 입력 필드 연결)
    const inputId = id || `input-${Math.random().toString(36).substr(2, 9)}`;

    return (
      <div className="w-full">
        {/* 라벨 렌더링 */}
        {label && (
          <label
            htmlFor={inputId}
            className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1"
          >
            {label}
          </label>
        )}

        {/* 입력 필드 컨테이너 (아이콘 포지셔닝을 위한 relative) */}
        <div className="relative">
          {/* 왼쪽 아이콘 */}
          {leftIcon && (
            <div className="absolute left-3 top-1/2 transform -translate-y-1/2 text-gray-400">
              {leftIcon}
            </div>
          )}

          {/* 메인 입력 필드 */}
          <input
            ref={ref}
            id={inputId}
            className={cn(
              // 기본 스타일
              "w-full transition-colors focus:outline-none",

              // variant 별 스타일
              variant === "default" &&
                "border-b-2 border-gray-200 bg-transparent focus:border-blue-500 dark:border-gray-600 dark:focus:border-blue-400",
              variant === "outlined" &&
                "border border-gray-300 rounded-lg bg-white focus:border-blue-500 focus:ring-2 focus:ring-blue-500/20 dark:bg-gray-900 dark:border-gray-700 dark:focus:border-blue-400",

              // 크기별 스타일
              inputSize === "sm" && "h-8 text-sm",
              inputSize === "md" && "h-10 text-base",
              inputSize === "lg" && "h-12 text-lg",
              // 아이콘에 따른 패딩 조정
              (leftIcon && !rightIcon && "pl-10 pr-3") ||
                (!leftIcon && rightIcon && "pl-3 pr-10") ||
                (leftIcon && rightIcon && "px-10") ||
                (!leftIcon && !rightIcon && "px-3"),
              // 에러 상태 스타일
              error && variant === "default" && "border-red-500 focus:border-red-500",
              error &&
                variant === "outlined" &&
                "border-red-500 focus:border-red-500 focus:ring-red-500/20",

              // 비활성 상태 스타일
              props.disabled && "opacity-50 cursor-not-allowed bg-gray-50 dark:bg-gray-800",

              // 추가 커스텀 클래스
              className
            )}
            {...props}
          />

          {/* 오른쪽 아이콘 */}
          {rightIcon && (
            <div className="absolute right-3 top-1/2 transform -translate-y-1/2 text-gray-400">
              {rightIcon}
            </div>
          )}
        </div>

        {/* 에러 메시지 또는 헬퍼 텍스트 */}
        {(error || helperText) && (
          <div className="mt-1 text-sm">
            {error ? (
              <span className="text-red-600 dark:text-red-400" role="alert">
                {error}
              </span>
            ) : (
              <span className="text-gray-500 dark:text-gray-400">{helperText}</span>
            )}
          </div>
        )}
      </div>
    );
  }
);

// React DevTools에서 컴포넌트 이름 표시
Input.displayName = "Input";

export default Input;
