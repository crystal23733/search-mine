import * as React from "react";
import { render, screen, fireEvent } from "@testing-library/react";
import Input from "../Input";

/**
 * 테스트용 아이콘 컴포넌트
 * 실제 아이콘 라이브러리 의존성 없이 테스트하기 위한 간단한 SVG
 */
const TestIcon = () => (
  <svg data-testid="icon" width="16" height="16" viewBox="0 0 16 16">
    <circle cx="8" cy="8" r="6" fill="currentColor" />
  </svg>
);

/**
 * Input 컴포넌트 단위 테스트
 *
 * 다양한 props 조합과 사용자 상호작용을 테스트하여
 * 컴포넌트의 정확한 동작을 보장합니다.
 */
describe("Input 컴포넌트", () => {
  /**
   * 라벨과 플레이스홀더가 정상적으로 렌더링되는지 테스트
   */
  it("라벨과 플레이스홀더를 올바르게 렌더링한다", () => {
    render(<Input label="사용자명" placeholder="닉네임을 입력해주세요" />);

    expect(screen.getByLabelText("사용자명")).toBeInTheDocument();
    expect(screen.getByPlaceholderText("닉네임을 입력해주세요")).toBeInTheDocument();
  });

  /**
   * variant prop에 따른 스타일 클래스가 올바르게 적용되는지 테스트
   */
  it("variant 속성에 따라 올바른 스타일을 적용한다", () => {
    const { rerender } = render(<Input variant="default" data-testid="input" />);
    expect(screen.getByTestId("input")).toHaveClass("border-b-2");

    rerender(<Input variant="outlined" data-testid="input" />);
    expect(screen.getByTestId("input")).toHaveClass("border", "rounded-lg");
  });

  /**
   * inputSize prop에 따른 크기 클래스가 올바르게 적용되는지 테스트
   */
  it("inputSize 속성에 따라 올바른 크기를 적용한다", () => {
    const { rerender } = render(<Input inputSize="sm" data-testid="input" />);
    expect(screen.getByTestId("input")).toHaveClass("h-8");

    rerender(<Input inputSize="md" data-testid="input" />);
    expect(screen.getByTestId("input")).toHaveClass("h-10");

    rerender(<Input inputSize="lg" data-testid="input" />);
    expect(screen.getByTestId("input")).toHaveClass("h-12");
  });

  /**
   * 에러 메시지가 표시되고 에러 스타일이 적용되는지 테스트
   */
  it("에러 메시지를 표시하고 에러 스타일을 적용한다", () => {
    render(<Input error="필수 입력 항목입니다" data-testid="input" />);

    const errorMessage = screen.getByText("필수 입력 항목입니다");
    expect(errorMessage).toBeInTheDocument();
    expect(errorMessage).toHaveAttribute("role", "alert");
    expect(screen.getByTestId("input")).toHaveClass("border-red-500");
  });

  /**
   * 헬퍼 텍스트가 올바르게 표시되는지 테스트
   */
  it("헬퍼 텍스트를 올바르게 표시한다", () => {
    render(<Input helperText="최소 3자 이상 입력해주세요" />);
    expect(screen.getByText("최소 3자 이상 입력해주세요")).toBeInTheDocument();
  });

  /**
   * 왼쪽/오른쪽 아이콘이 렌더링되고 적절한 패딩이 적용되는지 테스트
   */
  it("왼쪽과 오른쪽 아이콘을 렌더링하고 적절한 패딩을 적용한다", () => {
    render(
      <Input
        leftIcon={<TestIcon />}
        rightIcon={<TestIcon />}
        placeholder="아이콘과 함께"
        data-testid="input"
      />
    );

    expect(screen.getAllByTestId("icon")).toHaveLength(2);
    expect(screen.getByTestId("input")).toHaveClass("px-10");
  });

  /**
   * disabled 상태에서 올바른 스타일과 동작을 적용하는지 테스트
   */
  it("비활성 상태일 때 올바른 스타일과 동작을 적용한다", () => {
    render(<Input disabled placeholder="비활성 입력 필드" />);

    const input = screen.getByPlaceholderText("비활성 입력 필드");
    expect(input).toBeDisabled();
    expect(input).toHaveClass("opacity-50", "cursor-not-allowed");
  });

  /**
   * onChange 이벤트가 올바르게 호출되는지 테스트
   */
  it("값 변경 시 onChange 이벤트를 올바르게 호출한다", () => {
    const handleChange = jest.fn();
    render(<Input onChange={handleChange} placeholder="입력해보세요" />);

    const input = screen.getByPlaceholderText("입력해보세요");
    fireEvent.change(input, { target: { value: "테스트 입력" } });

    expect(handleChange).toHaveBeenCalledTimes(1);
    expect(handleChange).toHaveBeenCalledWith(
      expect.objectContaining({
        target: expect.objectContaining({ value: "테스트 입력" }),
      })
    );
  });

  /**
   * ref가 올바르게 전달되고 DOM 요소에 접근할 수 있는지 테스트
   */
  it("ref를 올바르게 전달하고 DOM 요소에 접근할 수 있다", () => {
    const ref = React.createRef<HTMLInputElement>();
    render(<Input ref={ref} placeholder="ref 테스트" />);

    expect(ref.current).toBeInstanceOf(HTMLInputElement);
    expect(ref.current?.placeholder).toBe("ref 테스트");

    // ref를 통한 포커스 동작 테스트
    ref.current?.focus();
    expect(ref.current).toHaveFocus();
  });

  /**
   * ID가 제공되지 않을 때 고유한 ID가 자동 생성되는지 테스트
   */
  it("ID가 제공되지 않을 때 고유한 ID를 자동 생성한다", () => {
    render(<Input label="자동 ID 테스트" />);

    const input = screen.getByLabelText("자동 ID 테스트");
    expect(input.id).toMatch(/^input-[a-z0-9]+$/);
  });

  /**
   * 커스텀 ID가 제공될 때 해당 ID를 사용하는지 테스트
   */
  it("커스텀 ID가 제공될 때 해당 ID를 사용한다", () => {
    render(<Input id="custom-input-id" label="커스텀 ID" />);

    const input = screen.getByLabelText("커스텀 ID");
    expect(input).toHaveAttribute("id", "custom-input-id");
  });

  /**
   * 에러와 헬퍼텍스트가 동시에 있을 때 에러만 표시되는지 테스트
   */
  it("에러와 헬퍼텍스트가 동시에 있을 때 에러만 표시한다", () => {
    render(<Input error="에러가 발생했습니다" helperText="이 텍스트는 표시되지 않습니다" />);

    expect(screen.getByText("에러가 발생했습니다")).toBeInTheDocument();
    expect(screen.queryByText("이 텍스트는 표시되지 않습니다")).not.toBeInTheDocument();
  });

  /**
   * 다크모드 클래스가 올바르게 적용되는지 테스트
   */
  it("다크모드 클래스를 올바르게 적용한다", () => {
    render(<Input variant="outlined" data-testid="input" />);

    const input = screen.getByTestId("input");
    expect(input).toHaveClass("dark:bg-gray-900", "dark:border-gray-700");
  });
});
