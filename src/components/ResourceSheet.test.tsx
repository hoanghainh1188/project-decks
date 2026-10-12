import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";

import { I18nProvider, setCurrentLang, type Lang } from "../i18n";
import { makeResource, standardCalendar } from "../test/fixtures";
import type { Overallocation, Resource } from "../types";
import { ResourceSheet } from "./ResourceSheet";

function renderSheet(resources: Resource[], overallocations: Overallocation[] = [], lang: Lang = "vi") {
  const handlers = {
    onAdd: vi.fn(),
    onChange: vi.fn(),
    onDelete: vi.fn(),
    onEditCalendar: vi.fn(),
    onCreateCalendar: vi.fn(),
  };
  render(
    <I18nProvider lang={lang}>
      <ResourceSheet
        resources={resources}
        calendars={[standardCalendar(1)]}
        overallocations={overallocations}
        {...handlers}
      />
    </I18nProvider>,
  );
  return { handlers, user: userEvent.setup() };
}

const an = makeResource({ uid: 10, name: "An" });
const crane = makeResource({ uid: 11, name: "Cẩu", kind: "machine" });
const over: Overallocation[] = [{ resource_uid: 10, date: "2026-01-05", units: 1.5, max_units: 1 }];

describe("ResourceSheet", () => {
  it("shows the Vietnamese headers and flags overallocated resources", () => {
    renderSheet([an, crane], over);
    const headers = screen.getAllByRole("columnheader").map((cell) => cell.textContent);
    expect(headers).toEqual([
      "Tên", "Loại", "Đơn vị tối đa", "Lịch", "Giá chuẩn / giờ", "Giá tăng ca", "Chi phí mỗi lần", "Quá tải", "",
    ]);
    expect(screen.getByText("Có")).toBeInTheDocument();
    expect(screen.getByText("Không")).toBeInTheDocument();
    expect(screen.getByLabelText("Lịch An")).toBeInTheDocument();
  });

  it("says when there are no resources", () => {
    renderSheet([]);
    expect(screen.getByText("Chưa có nguồn lực.")).toBeInTheDocument();
  });
});

describe("ResourceSheet in English", () => {
  afterEach(() => setCurrentLang("vi"));

  it("uses Project's English column names", () => {
    renderSheet([an, crane], over, "en");
    const headers = screen.getAllByRole("columnheader").map((cell) => cell.textContent);
    expect(headers).toEqual([
      "Name", "Type", "Max units", "Calendar", "Std. rate / hr", "Ovt. rate", "Cost/use", "Overallocated", "",
    ]);
    expect(screen.getByText("Yes")).toBeInTheDocument();
    expect(screen.getByText("No")).toBeInTheDocument();
    expect(screen.getAllByRole("option", { name: "Person" })).toHaveLength(2);
    expect(screen.getAllByRole("option", { name: "Equipment" })).toHaveLength(2);
    expect(screen.getByLabelText("An calendar")).toBeInTheDocument();
    expect(screen.getAllByRole("option", { name: "Project calendar" })).toHaveLength(2);
    expect(screen.getAllByRole("button", { name: "Edit" })).toHaveLength(2);
    expect(screen.getAllByRole("button", { name: "New calendar" })).toHaveLength(2);
  });

  it("adds and deletes resources from English buttons", async () => {
    const { handlers, user } = renderSheet([an], [], "en");
    await user.click(screen.getByRole("button", { name: "Add person" }));
    await user.click(screen.getByRole("button", { name: "Add equipment" }));
    await user.click(screen.getByRole("button", { name: "Delete" }));
    expect(handlers.onAdd.mock.calls).toEqual([["person"], ["machine"]]);
    expect(handlers.onDelete).toHaveBeenCalledExactlyOnceWith(10);
  });

  it("says when there are no resources", () => {
    renderSheet([], [], "en");
    expect(screen.getByText("No resources yet.")).toBeInTheDocument();
  });
});
