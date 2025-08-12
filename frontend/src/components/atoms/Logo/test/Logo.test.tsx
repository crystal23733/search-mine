import * as React from "react";
import { render, screen, fireEvent } from "@testing-library/react";
import Logo from "../Logo";

// Lucide React 아이콘 mock
jest.mock("lucide-react", () => ({
  Grid3X3: ({ className }: { className: string }) => (
    <div data-testid="grid-icon" className={className} />
  ),
  Zap: ({ className }: { className: string }) => (
    <div data-testid="zap-icon" className={className} />
  ),
}));

describe("Logo 컴포넌트", () => {
  it("기본 로고를 아이콘과 텍스트와 함께 렌더링한다", () => {
    render(<Logo />);

    expect(screen.getByText("Mines")).toBeInTheDocument();
    expect(screen.getByText("BATTLE")).toBeInTheDocument();
    expect(screen.getByTestId("grid-icon")).toBeInTheDocument();
    expect(screen.getByTestId("zap-icon")).toBeInTheDocument();
  });

  it("showText가 false일 때 텍스트를 숨긴다", () => {
    render(<Logo showText={false} />);

    expect(screen.queryByText("Mines")).not.toBeInTheDocument();
    expect(screen.queryByText("BATTLE")).not.toBeInTheDocument();
    expect(screen.getByTestId("grid-icon")).toBeInTheDocument();
  });

  it("onClick이 제공될 때만 button role과 접근성 속성을 설정한다", () => {
    const { rerender } = render(<Logo data-testid="logo" />);

    // onClick이 없을 때
    let logo = screen.getByTestId("logo");
    expect(logo).not.toHaveAttribute("role");
    expect(logo).not.toHaveAttribute("tabIndex");
    expect(logo).not.toHaveAttribute("aria-label");

    // onClick이 있을 때
    const handleClick = jest.fn();
    rerender(<Logo onClick={handleClick} data-testid="logo" />);

    logo = screen.getByTestId("logo");
    expect(logo).toHaveAttribute("role", "button");
    expect(logo).toHaveAttribute("tabIndex", "0");
    expect(logo).toHaveAttribute("aria-label", "Mines Battle 홈으로 이동");
  });

  it("커스텀 aria-label을 올바르게 적용한다", () => {
    const handleClick = jest.fn();
    render(<Logo onClick={handleClick} aria-label="메인 페이지로 이동" data-testid="logo" />);

    expect(screen.getByTestId("logo")).toHaveAttribute("aria-label", "메인 페이지로 이동");
  });

  it("클릭 시 onClick 핸들러를 호출한다", () => {
    const handleClick = jest.fn();
    render(<Logo onClick={handleClick} data-testid="logo" />);

    fireEvent.click(screen.getByTestId("logo"));
    expect(handleClick).toHaveBeenCalledTimes(1);
  });

  it("키보드 접근성이 올바르게 작동한다", () => {
    const handleClick = jest.fn();
    render(<Logo onClick={handleClick} data-testid="logo" />);

    const logo = screen.getByTestId("logo");

    // Enter 키
    fireEvent.keyDown(logo, { key: "Enter" });
    expect(handleClick).toHaveBeenCalledTimes(1);

    // Space 키
    fireEvent.keyDown(logo, { key: " " });
    expect(handleClick).toHaveBeenCalledTimes(2);

    // 다른 키는 동작하지 않음
    fireEvent.keyDown(logo, { key: "Escape" });
    expect(handleClick).toHaveBeenCalledTimes(2);
  });

  it("아이콘 컨테이너에 aria-hidden이 적용된다", () => {
    render(<Logo data-testid="logo" />);

    const iconContainer = screen.getByTestId("grid-icon").parentElement;
    expect(iconContainer).toHaveAttribute("aria-hidden", "true");
  });

  it("포커스 스타일이 올바르게 적용된다", () => {
    const handleClick = jest.fn();
    render(<Logo onClick={handleClick} data-testid="logo" />);

    const logo = screen.getByTestId("logo");
    expect(logo).toHaveClass("focus:outline-none", "focus:ring-2", "focus:ring-blue-500");
  });
});
