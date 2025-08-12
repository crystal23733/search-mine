/**
 * Divider 컴포넌트의 Props 인터페이스
 */
export default interface DividerProps {
  /**
   * 구분선의 방향
   * - horizontal: 가로 방향
   * - vertical: 세로 방향
   * @default "horizontal"
   */
  orientation?: "horizontal" | "vertical";

  /**
   * 구분선의 스타일
   * - solid: 실선
   * - dashed: 점선
   * - dotted: 점선
   * - gradient: 그라데이션
   * @default "solid"
   */
  variant?: "solid" | "dashed" | "dotted" | "gradient";

  /**
   * 구분선의 두께
   * - thin: 얇은 두께
   * - medium: 중간 두께
   * - thick: 두꺼운 두께
   * @default "medium"
   */
  thickness?: "thin" | "medium" | "thick";

  /**
   * 구분선에 표시할 텍스트
   */
  text?: string;

  /**
   * 텍스트 위치 (horizontal 방향에만 적용)
   */
  textPosition?: "center" | "left" | "right";

  /**
   * 구분선의 색상
   * - default: 기본 색상
   * - primary: 주요 색상
   * - secondary: 보조 색상
   * - muted: 어두운 색상
   * @default "default"
   */
  color?: "default" | "primary" | "secondary" | "muted";

  /**
   * 구분선 주변 간격
   * - none: 간격 없음
   * - sm: 작은 간격
   * - md: 중간 간격
   * - lg: 큰 간격
   * - xl: 매우 큰 간격
   * @default "md"
   */
  spacing?: "none" | "sm" | "md" | "lg" | "xl";

  /**
   * 추가 CSS 클래스
   */
  className?: string;

  /**
   * 컴포넌트 타입
   * - div: 일반 div 태그
   * - hr: hr 태그
   * @default "div"
   */
  as?: "div" | "hr";
}
