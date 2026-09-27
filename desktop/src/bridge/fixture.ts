import type { AbilityView, CardView, CellView, MatchView, PieceView, PlayerId, Position, Preset } from "./types";

const labels: Record<string, string> = {
  pawn: "Pa", rook: "Ro", knight: "Kn", bishop: "Bi", queen: "Qu", king: "Ki",
  magician: "Mg", dragon: "Dr", paladin: "Pl", archer: "Ar", builder: "Bu", wall: "Wa",
};

const step: AbilityView = {
  id: "orthogonal-step", name: "Orthogonal step", description: "Choose an adjacent empty square",
  actions: 1, mana: 0, cooldown: 2, available: true,
};

function cellMask(preset: Preset, width: number, height: number): CellView[] {
  const corner = preset === "cross14" ? 3 : preset === "cross24" ? 4 : 0;
  const cells: CellView[] = [];
  for (let y = 0; y < height; y++) for (let x = 0; x < width; x++) {
    const cut = corner > 0 && (x < corner || x >= width - corner) && (y < corner || y >= height - corner);
    cells.push({ position: { x, y }, playable: !cut, magic: !cut && (x + y) % 13 === 0, buildable: !cut });
  }
  return cells;
}

function piece(id: number, archetype: string, owner: PlayerId, x: number, y: number): PieceView {
  return {
    id: String(id), archetype, label: labels[archetype] ?? "??", owner, position: { x, y },
    damage: 0, tough: archetype === "dragon" ? 3 : 1, acted: false,
    tags: archetype === "king" ? ["Royal", "Biological"] : archetype === "wall" ? ["Structure", "Impenetrable 3"] : ["Biological"],
    statuses: archetype === "magician" ? [{ kind: "Fire", remaining: 3 }] : [],
    abilities: archetype === "bishop" ? [step] : archetype === "king" ? [{ id: "teleport", name: "Teleport", description: "Move to an empty square within range 5", actions: 1, mana: 2, available: false, reason: "Insufficient mana" }] : [],
  };
}

function army(width: number, height: number, four: boolean): PieceView[] {
  const pieces: PieceView[] = [];
  let id = 1;
  const back = ["rook", "knight", "bishop", "queen", "king", "bishop", "knight", "rook"];
  const span = Math.min(8, width);
  const offset = Math.floor((width - span) / 2);
  for (let i = 0; i < span; i++) {
    pieces.push(piece(id++, back[i], 0, offset + i, 0), piece(id++, "pawn", 0, offset + i, 1));
    pieces.push(piece(id++, back[i], 1, offset + i, height - 1), piece(id++, "pawn", 1, offset + i, height - 2));
  }
  if (width > 8) {
    pieces.push(piece(id++, "dragon", 0, Math.floor(width / 2), 3));
    pieces.push(piece(id++, "magician", 1, Math.floor(width / 2), height - 4));
  }
  if (four) {
    const mid = Math.floor(height / 2);
    for (let y = mid - 3; y <= mid + 3; y++) {
      pieces.push(piece(id++, y === mid ? "king" : "pawn", 2, 0, y));
      pieces.push(piece(id++, y === mid ? "king" : "pawn", 3, width - 1, y));
    }
  }
  return pieces;
}

const hand: CardView[] = [
  { id: "c1", name: "Summon Knight", kind: "summon", cost: 1, description: "Summon a Knight on a legal square.", available: true, summon: "knight" },
  { id: "c2", name: "Forced March", kind: "utility", cost: 0, description: "Reset one friendly piece's action.", available: true },
  { id: "c3", name: "Fire", kind: "onboard", cost: 1, description: "Unlock the Magician's Fire ability.", available: false, reason: "Played to the on-board zone" },
];

export function makeDemoMatch(preset: Preset): MatchView {
  const sizes: Record<Preset, [number, number]> = { classic: [8, 8], expansion: [16, 17], cross14: [14, 14], cross24: [24, 24], abilities: [8, 8], cards: [8, 8], siege: [8, 8], events: [8, 8], catalogue: [8, 8] };
  const [width, height] = sizes[preset];
  return {
    sessionId: "demo-session", revision: "1", preset, width, height,
    cells: cellMask(preset, width, height), pieces: army(width, height, preset.startsWith("cross")),
    players: [
      { id: 0, name: "Gold", mana: 3, actions: 1 },
      { id: 1, name: "Azure", mana: 2, actions: 1, ai: "Heuristic AI" },
      ...(preset.startsWith("cross") ? [{ id: 2 as PlayerId, name: "Violet", mana: 1, actions: 1 }, { id: 3 as PlayerId, name: "Coral", mana: 1, actions: 1, ai: "First AI" }] : []),
    ],
    currentPlayer: 0, turn: 12, hand, onboard: [hand[2]],
    log: [
      { id: "l1", turn: 12, text: "Gold's turn began" },
      { id: "l2", turn: 11, text: "Azure moved Knight to f6" },
      { id: "l3", turn: 11, text: "Gold played Forced March" },
    ],
  };
}

export function positionKey(p: Position) { return `${p.x}:${p.y}`; }
