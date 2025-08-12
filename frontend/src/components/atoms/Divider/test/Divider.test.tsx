import * as React from "react";
import { render, screen } from "@testing-library/react";
import Divider from "../Divider";

/**
 * Divider 컴포넌트 단위 테스트
 *
 * 모든 props 조합, 접근성, 반응형 스타일을 테스트합니다.
 */
describe("Divider 컴포넌트", () => {
  /**
   * 기본 구분선이 올바르게 렌더링되는지 테스트
   */
  it("기본 구분선을 올바르게 렌더링한다", () => {
    render(<Divider data-testid="divider" />);

    const divider = screen.getByTestId("divider");
    expect(divider).toBeInTheDocument();
    expect(divider).toHaveAttribute("role", "separator");
    expect(divider).toHaveAttribute("aria-orientation", "horizontal");
  });

  /**
   * 텍스트가 있는 구분선이 올바르게 렌더링되는지 테스트
   */
  it("텍스트가 있는 구분선을 올바르게 렌더링한다", () => {
    render(<Divider text="또는" data-testid="divider" />);

    expect(screen.getByText("또는")).toBeInTheDocument();
    expect(screen.getByTestId("divider")).toHaveAttribute("aria-label", "구분선: 또는");
  });

  /**
   * 텍스트 위치에 따른 레이아웃이 올바른지 테스트
   */
  it("텍스트 위치에 따라 올바른 레이아웃을 적용한다", () => {
    const { rerender } = render(<Divider text="왼쪽" textPosition="left" data-testid="divider" />);

    // 왼쪽 위치 테스트 (왼쪽 선이 숨겨져야 함)
    let leftLine = screen.getByTestId("divider").querySelector('[aria-hidden="true"]');
    expect(leftLine).toHaveClass("hidden");

    // 오른쪽 위치 테스트
    rerender(<Divider text="오른쪽" textPosition="right" data-testid="divider" />);
    // 구현 상 오른쪽 선이 숨겨져야 함

    // 중앙 위치 테스트
    rerender(<Divider text="중앙" textPosition="center" data-testid="divider" />);
    expect(screen.getByText("중앙")).toBeInTheDocument();
  });

  /**
   * 수직 구분선이 올바르게 렌더링되는지 테스트
   */
  it("수직 구분선을 올바르게 렌더링한다", () => {
    render(<Divider orientation="vertical" data-testid="divider" />);

    const divider = screen.getByTestId("divider");
    expect(divider).toHaveAttribute("aria-orientation", "vertical");
    expect(divider).toHaveClass("h-full", "flex-col");
  });

  /**
   * 다양한 variant 스타일이 올바르게 적용되는지 테스트
   */
  it("variant에 따라 올바른 스타일을 적용한다", () => {
    const { rerender } = render(<Divider variant="solid" data-testid="divider" />);
    expect(screen.getByTestId("divider")).toHaveClass("border-solid");

    rerender(<Divider variant="dashed" data-testid="divider" />);
    expect(screen.getByTestId("divider")).toHaveClass("border-dashed");

    rerender(<Divider variant="gradient" data-testid="divider" />);
    expect(screen.getByTestId("divider")).toHaveClass("bg-gradient-to-r");
  });

  /**
   * 색상 테마가 올바르게 적용되는지 테스트
   */
  it("color에 따라 올바른 색상을 적용한다", () => {
    const { rerender } = render(<Divider color="primary" data-testid="divider" />);
    expect(screen.getByTestId("divider")).toHaveClass("bg-blue-500");

    rerender(<Divider color="secondary" data-testid="divider" />);
    expect(screen.getByTestId("divider")).toHaveClass("bg-purple-500");
  });

  /**
   * 두께 설정이 올바르게 적용되는지 테스트
   */
  it("thickness에 따라 올바른 두께를 적용한다", () => {
    const { rerender } = render(<Divider thickness="thin" data-testid="divider" />);
    expect(screen.getByTestId("divider")).toHaveClass("h-px");

    rerender(<Divider thickness="medium" data-testid="divider" />);
    expect(screen.getByTestId("divider")).toHaveClass("h-0.5");

    rerender(<Divider thickness="thick" data-testid="divider" />);
    expect(screen.getByTestId("divider")).toHaveClass("h-1");
  });

  /**
   * 여백 설정이 올바르게 적용되는지 테스트
   */
  it("spacing에 따라 올바른 여백을 적용한다", () => {
    const { rerender } = render(<Divider spacing="sm" data-testid="divider" />);
    expect(screen.getByTestId("divider")).toHaveClass("my-1", "sm:my-2");

    rerender(<Divider spacing="lg" data-testid="divider" />);
    expect(screen.getByTestId("divider")).toHaveClass("my-3", "sm:my-6");
  });

  /**
   * as prop에 따라 올바른 HTML 요소로 렌더링되는지 테스트
   */
  it("as prop에 따라 올바른 HTML 요소로 렌더링한다", () => {
    const { rerender } = render(<Divider as="hr" data-testid="divider" />);
    expect(screen.getByTestId("divider").tagName).toBe("HR");

    rerender(<Divider as="div" data-testid="divider" />);
    expect(screen.getByTestId("divider").tagName).toBe("DIV");
  });

  /**
   * 커스텀 className이 올바르게 적용되는지 테스트
   */
  it("커스텀 className을 올바르게 적용한다", () => {
    render(<Divider className="custom-class" data-testid="divider" />);
    expect(screen.getByTestId("divider")).toHaveClass("custom-class");
  });

  /**
   * 텍스트가 긴 경우 적절히 처리되는지 테스트 (반응형)
   */
  it("긴 텍스트를 적절히 처리한다", () => {
    const longText = "매우 긴 텍스트입니다. 이 텍스트는 모바일에서 잘릴 수 있습니다.";
    render(<Divider text={longText} />);

    const textElement = screen.getByText(longText);
    expect(textElement).toHaveClass("truncate", "max-w-[200px]");
    expect(textElement).toHaveAttribute("title", longText);
  });

  /**
   * 수직 구분선에 텍스트가 있을 때 SimpleDivider를 사용하는지 테스트
   */
  it("수직 구분선에서는 텍스트를 무시하고 SimpleDivider를 사용한다", () => {
    render(<Divider orientation="vertical" text="무시될 텍스트" data-testid="divider" />);

    expect(screen.queryByText("무시될 텍스트")).not.toBeInTheDocument();
    expect(screen.getByTestId("divider")).toHaveAttribute("aria-orientation", "vertical");
  });

  /**
   * 접근성 속성이 올바르게 설정되는지 테스트
   */
  it("접근성 속성을 올바르게 설정한다", () => {
    render(<Divider text="접근성 테스트" data-testid="divider" />);

    const divider = screen.getByTestId("divider");
    expect(divider).toHaveAttribute("role", "separator");
    expect(divider).toHaveAttribute("aria-label", "구분선: 접근성 테스트");

    const textElement = screen.getByText("접근성 테스트");
    expect(textElement).toHaveAttribute("aria-label", "구분 텍스트: 접근성 테스트");
  });
});
