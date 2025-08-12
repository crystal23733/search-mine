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
   * 접근성을 위한 alt 텍스트
   * @default "Mines Battle 로고"
   */
  alt?: string;
}
