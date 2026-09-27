import { describe, expect, it } from "vitest";
import { makeDemoMatch, positionKey } from "./fixture";

describe("desktop bridge fixture", () => {
  it.each([
    ["classic", 8, 8, 64],
    ["expansion", 16, 17, 272],
    ["cross14", 14, 14, 160],
    ["cross24", 24, 24, 512],
  ] as const)("builds the %s playable-cell mask", (preset, width, height, playable) => {
    const match = makeDemoMatch(preset);
    expect([match.width, match.height]).toEqual([width, height]);
    expect(match.cells.filter((cell) => cell.playable)).toHaveLength(playable);
    expect(new Set(match.cells.map((cell) => positionKey(cell.position))).size).toBe(width * height);
  });

  it("does not expose another seat's hand", () => {
    const match = makeDemoMatch("classic");
    expect(match.hand).toHaveLength(3);
    expect(match.players.every((player) => !("hand" in player))).toBe(true);
  });
});
