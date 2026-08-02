import { render, screen } from "@testing-library/react";
import { expect, test } from "vitest";
import { Badge } from "./badge";

test("Badge renders its label and variant class", () => {
  render(<Badge variant="vendored">vendored</Badge>);
  const el = screen.getByText("vendored");
  expect(el).toBeTruthy();
  expect(el.className).toContain("bg-sky-100");
});
