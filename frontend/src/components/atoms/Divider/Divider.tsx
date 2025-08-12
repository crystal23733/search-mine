import { memo } from "react";
import { TextDivider } from "./TextDivider";
import { SimpleDivider } from "./SimpleDivider";
import DividerProps from "./Divider.interface";

/**
 * 구분선 컴포넌트
 *
 * 콘텐츠 영역을 시각적으로 구분하는 다양한 스타일의 구분선을 제공합니다.
 * 텍스트 포함, 방향, 색상, 스타일 등을 자유롭게 커스터마이징할 수 있으며,
 * 반응형 디자인과 접근성을 완벽 지원합니다.
 *
 * @component
 * @example
 * // 기본 수평 구분선
 * <Divider />
 *
 * @example
 * // 텍스트가 있는 구분선
 * <Divider text="또는" />
 *
 * @example
 * // 로그인 폼의 소셜/일반 로그인 구분
 * <Divider text="소셜 계정으로 로그인" color="primary" />
 *
 * @example
 * // 수직 구분선 (네비게이션 메뉴)
 * <Divider orientation="vertical" thickness="medium" />
 *
 * @example
 * // 그라디언트 구분선
 * <Divider variant="gradient" spacing="lg" />
 *
 * @example
 * // 왼쪽 정렬 텍스트
 * <Divider text="설정" textPosition="left" color="muted" />
 *
 * @param props - Divider 컴포넌트의 props
 * @returns 조건에 맞는 구분선 컴포넌트
 */
const Divider = memo((props: DividerProps) => {
  const { text, orientation } = props;

  // 텍스트가 있고 수평 방향인 경우 TextDivider 사용
  if (text && orientation !== "vertical") {
    return <TextDivider {...props} text={text} />;
  }

  // 일반 구분선은 SimpleDivider 사용
  return <SimpleDivider {...props} />;
});

Divider.displayName = "Divider";

export default Divider;
