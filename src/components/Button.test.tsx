import { render, screen } from "@testing-library/react";
import { Trash2 } from "lucide-react";
import { describe, expect, it } from "vitest";

import { Button } from "./Button";

describe("Button", () => {
  it("is a plain button named by its text, with a decorative icon", () => {
    render(
      <Button variant="danger" Icon={Trash2}>
        Xóa
      </Button>,
    );
    const button = screen.getByRole("button", { name: "Xóa" });
    expect(button).toHaveAttribute("type", "button");
    expect(button).toHaveClass("btn", "btn-danger");
    expect(button.querySelector("svg")).toHaveAttribute("aria-hidden", "true");
  });

  it("defaults to secondary", () => {
    render(<Button>Hủy</Button>);
    expect(screen.getByRole("button", { name: "Hủy" })).toHaveClass("btn-secondary");
  });
});
