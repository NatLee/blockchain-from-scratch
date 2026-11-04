import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";
import App, { ROUTES } from "../src/App";
import { core } from "../src/lib/core";

afterEach(() => {
  cleanup();
  window.location.hash = "";
});

async function open(path: string) {
  window.location.hash = `#/${path}`;
  render(<App />);
  // 等頁面裡非同步的工作（例如建立一條挖好礦的鏈）做完。
  await act(async () => {
    await new Promise((resolve) => setTimeout(resolve, 50));
  });
}

describe("每一頁都畫得出來", () => {
  for (const route of ROUTES) {
    it(`#/${route.path}`, async () => {
      await open(route.path);
      expect(screen.getAllByRole("heading", { level: 1 }).length).toBeGreaterThan(0);
    });
  }
});

describe("互動", () => {
  it("the hash page shows the SHA-256 of what you type", async () => {
    await open("hash");
    const [inputA] = screen.getAllByRole("textbox");
    fireEvent.change(inputA, { target: { value: "abc" } });
    expect(screen.getByText(core.sha256Hex("abc"))).toBeTruthy();
  });

  it("the chain page notices tampering", async () => {
    await open("chain");
    expect(await screen.findByText(/整條鏈都正常/)).toBeTruthy();
    fireEvent.change(screen.getByLabelText("區塊 1 的內容"), { target: { value: "Alice 付給 Bob 1000 元" } });
    expect(screen.getByText(/鏈被改過了/)).toBeTruthy();
    expect(screen.getByText("data_hash 對不上內容：內容被改過")).toBeTruthy();
  });
});
