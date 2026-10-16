import { fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { I18nProvider, setCurrentLang } from "../i18n";
import { MoneyInput } from "./MoneyInput";

afterEach(() => setCurrentLang("vi"));

function renderInput(value: number | null, props: { optional?: boolean; lang?: "vi" | "en" } = {}) {
  const onCommit = vi.fn();
  render(
    <I18nProvider lang={props.lang ?? "vi"}>
      <MoneyInput ariaLabel="Chi phí" value={value} optional={props.optional} onCommit={onCommit} />
    </I18nProvider>,
  );
  return { onCommit, input: screen.getByLabelText("Chi phí") };
}

describe("MoneyInput", () => {
  it("shows the amount grouped and reads it back the same way", () => {
    const { onCommit, input } = renderInput(1_248_000);
    expect(input).toHaveValue("1.248.000");
    fireEvent.change(input, { target: { value: "2.500.000" } });
    fireEvent.blur(input);
    expect(onCommit).toHaveBeenCalledWith(2_500_000);
  });

  it("sends nothing for an unchanged amount", () => {
    const { onCommit, input } = renderInput(1_248_000);
    fireEvent.change(input, { target: { value: "1248000" } });
    fireEvent.blur(input);
    expect(onCommit).not.toHaveBeenCalled();
  });

  it("keeps the old amount and says why when the text does not read, instead of saving 0", () => {
    const { onCommit, input } = renderInput(1_248_000);
    fireEvent.change(input, { target: { value: "1.2.3" } });
    fireEvent.blur(input);
    expect(onCommit).not.toHaveBeenCalled();
    expect(input).toHaveValue("1.248.000");
    expect(input).toHaveAttribute("aria-invalid", "true");
    expect(input).toHaveAccessibleDescription(/Không đọc được số tiền/);
  });

  it("clears an optional amount and refuses to clear one that is not", () => {
    const optional = renderInput(500, { optional: true });
    fireEvent.change(optional.input, { target: { value: "" } });
    fireEvent.blur(optional.input);
    expect(optional.onCommit).toHaveBeenCalledWith(null);
  });

  it("speaks English grouping", () => {
    const { onCommit, input } = renderInput(65_000.5, { lang: "en" });
    expect(input).toHaveValue("65,000.5");
    fireEvent.change(input, { target: { value: "1,000" } });
    fireEvent.blur(input);
    expect(onCommit).toHaveBeenCalledWith(1000);
  });
});
