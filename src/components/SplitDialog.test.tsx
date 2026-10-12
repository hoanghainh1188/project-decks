import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";

import { I18nProvider, setCurrentLang } from "../i18n";
import { at, makeTask } from "../test/fixtures";
import { SplitDialog } from "./SplitDialog";

const task = makeTask({ uid: 3, name: "Viết", start: at(2026, 1, 5, 8), finish: at(2026, 1, 7, 17), duration: 3 * 480 });

describe("SplitDialog", () => {
  afterEach(() => setCurrentLang("vi"));

  it("shows Vietnamese by default and rejects an empty gap", async () => {
    const user = userEvent.setup();
    const onSubmit = vi.fn(async () => true);
    render(<SplitDialog task={task} onClose={() => {}} onSubmit={onSubmit} />);
    expect(screen.getByRole("heading", { name: "Tách công việc" })).toBeInTheDocument();
    expect(screen.getByLabelText("Khoảng ngắt")).toHaveValue("1 ngày");
    await user.clear(screen.getByLabelText("Khoảng ngắt"));
    await user.click(screen.getByRole("button", { name: "Tách" }));
    expect(screen.getByText("Khoảng ngắt phải lớn hơn 0, ví dụ 1 ngày, 4 giờ, 30 phút")).toBeInTheDocument();
    expect(onSubmit).not.toHaveBeenCalled();
  });

  it("shows English text and submits the parsed gap", async () => {
    const user = userEvent.setup();
    const onSubmit = vi.fn(async () => true);
    const onClose = vi.fn();
    render(
      <I18nProvider lang="en">
        <SplitDialog task={task} onClose={onClose} onSubmit={onSubmit} />
      </I18nProvider>,
    );
    expect(screen.getByRole("heading", { name: "Split Task" })).toBeInTheDocument();
    expect(screen.getByText("Viết · 2026-01-05 08:00 – 2026-01-07 17:00 · duration 3 days")).toBeInTheDocument();
    expect(screen.getByText("Split at")).toBeInTheDocument();
    expect(screen.getByLabelText("Split point")).toBeInTheDocument();
    expect(screen.getByText("Gap (working time)")).toBeInTheDocument();
    expect(screen.getByLabelText("Gap")).toHaveValue("1 day");
    expect(screen.getByRole("button", { name: "Cancel" })).toBeInTheDocument();

    await user.clear(screen.getByLabelText("Gap"));
    await user.click(screen.getByRole("button", { name: "Split" }));
    expect(screen.getByText("The gap must be greater than 0, for example 1 day, 4 hrs, 30 mins")).toBeInTheDocument();

    await user.type(screen.getByLabelText("Gap"), "4 hrs");
    await user.click(screen.getByRole("button", { name: "Split" }));
    expect(onSubmit).toHaveBeenCalledWith(expect.any(Number), 240);
    expect(onClose).toHaveBeenCalled();
  });
});
