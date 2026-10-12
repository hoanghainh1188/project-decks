import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import { MenuButton } from "./MenuButton";

function setup() {
  const task = vi.fn();
  const milestone = vi.fn();
  render(
    <>
      <MenuButton
        label="Thêm việc"
        items={[
          { label: "Công việc", onSelect: task },
          { label: "Mốc", onSelect: milestone },
          { label: "Công việc lặp…", onSelect: vi.fn(), disabled: true, title: "Chưa mở dự án" },
        ]}
      />
      <button type="button">Bên ngoài</button>
    </>,
  );
  return { user: userEvent.setup(), task, milestone };
}

describe("MenuButton", () => {
  it("opens on click, focuses the first item, and runs the chosen one", async () => {
    const { user, milestone } = setup();
    const button = screen.getByRole("button", { name: /Thêm việc/ });
    expect(button).toHaveAttribute("aria-expanded", "false");
    await user.click(button);
    expect(button).toHaveAttribute("aria-expanded", "true");
    expect(screen.getByRole("menuitem", { name: "Công việc" })).toHaveFocus();
    await user.click(screen.getByRole("menuitem", { name: "Mốc" }));
    expect(milestone).toHaveBeenCalledOnce();
    expect(screen.queryByRole("menu")).toBeNull();
    expect(button).toHaveFocus();
  });

  it("moves with the arrow keys over enabled items and closes on Escape", async () => {
    const { user, task } = setup();
    const button = screen.getByRole("button", { name: /Thêm việc/ });
    button.focus();
    await user.keyboard("{ArrowDown}");
    expect(screen.getByRole("menuitem", { name: "Công việc" })).toHaveFocus();
    await user.keyboard("{ArrowDown}");
    expect(screen.getByRole("menuitem", { name: "Mốc" })).toHaveFocus();
    // The disabled item is skipped, so the next step wraps to the top.
    await user.keyboard("{ArrowDown}");
    expect(screen.getByRole("menuitem", { name: "Công việc" })).toHaveFocus();
    await user.keyboard("{Enter}");
    expect(task).toHaveBeenCalledOnce();
    await user.click(button);
    await user.keyboard("{Escape}");
    expect(screen.queryByRole("menu")).toBeNull();
    expect(button).toHaveFocus();
  });

  it("closes when the user clicks elsewhere", async () => {
    const { user } = setup();
    await user.click(screen.getByRole("button", { name: /Thêm việc/ }));
    await user.click(screen.getByRole("button", { name: "Bên ngoài" }));
    expect(screen.queryByRole("menu")).toBeNull();
  });
});
