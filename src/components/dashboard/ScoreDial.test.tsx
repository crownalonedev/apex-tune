import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { ScoreDial } from "./ScoreDial";

describe("ScoreDial", () => {
  it("announces the score and what it's based on", () => {
    render(<ScoreDial score={{ value: 72, counted: 7, applied: 5 }} />);
    expect(screen.getByRole("img", { name: "Optimization score: 72 out of 100" })).toBeInTheDocument();
    expect(screen.getByText("5 of 7 recommended tweaks applied")).toBeInTheDocument();
  });

  it("handles nothing to score", () => {
    render(<ScoreDial score={{ value: null, counted: 0, applied: 0 }} />);
    expect(screen.getByRole("img", { name: /nothing to score yet/ })).toBeInTheDocument();
  });
});
