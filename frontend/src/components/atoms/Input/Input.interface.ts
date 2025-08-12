import { InputHTMLAttributes } from "react";

/**
 * Input 컴포넌트의 Props 인터페이스
 *
 * @interface InputProps
 * @extends {InputHTMLAttributes<HTMLInputElement>}
 */
export default interface InputProps extends InputHTMLAttributes<HTMLInputElement> {
  /**
   * 입력 필드 위에 표시될 라벨 텍스트
   * @example "사용자명", "이메일 주소"
   */
  label?: string;

  /**
   * 유효성 검사 실패 시 표시될 에러 메시지
   * 에러가 있을 경우 입력 필드가 빨간색으로 강조됩니다
   * @example "필수 입력 항목입니다", "이메일 형식이 올바르지 않습니다"
   */
  error?: string;

  /**
   * 입력 필드 아래에 표시될 도움말 텍스트
   * 에러가 없을 때만 표시됩니다
   * @example "최소 3자 이상 입력해주세요", "영문, 숫자 조합으로 입력"
   */
  helperText?: string;

  /**
   * 입력 필드의 시각적 스타일 변형
   * - default: 밑줄 스타일 (미니멀)
   * - outlined: 테두리 스타일 (명확한 구분)
   * @default "default"
   */
  variant?: "default" | "outlined";

  /**
   * 입력 필드의 크기
   * - sm: 작은 크기 (높이 32px)
   * - md: 중간 크기 (높이 40px)
   * - lg: 큰 크기 (높이 48px)
   * @default "md"
   */
  inputSize?: "sm" | "md" | "lg";

  /**
   * 입력 필드 왼쪽에 표시될 아이콘 또는 요소
   * @example <FiUser />, <SearchIcon />
   */
  leftIcon?: React.ReactNode;

  /**
   * 입력 필드 오른쪽에 표시될 아이콘 또는 요소
   * @example <FiEye />, <ClearButton />
   */
  rightIcon?: React.ReactNode;
}
