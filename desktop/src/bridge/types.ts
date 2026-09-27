export type PlayerId = 0 | 1 | 2 | 3;
export type Preset = "classic" | "expansion" | "cross14" | "cross24" | "abilities" | "cards" | "siege" | "events" | "catalogue";
export type ActionKind = "move" | "take" | "attack" | "ability";
export type UiMode = "idle" | "piece" | "action" | "target" | "preview" | "submitting";

export interface Position { x: number; y: number }
export interface StatusView { kind: string; remaining: number }
export interface AbilityView {
  id: string;
  name: string;
  description: string;
  actions: number;
  mana: number;
  cooldown?: number;
  available: boolean;
  reason?: string;
}
export interface PieceView {
  id: string;
  archetype: string;
  label: string;
  owner: PlayerId;
  position: Position;
  damage: number;
  tough: number;
  acted: boolean;
  tags: string[];
  statuses: StatusView[];
  abilities: AbilityView[];
}
export interface CellView { position: Position; playable: boolean; magic?: boolean; buildable?: boolean }
export interface TargetView { position: Position; kind: ActionKind; choiceId: string }
export interface CardView {
  id: string;
  name: string;
  kind: "summon" | "utility" | "onboard";
  cost: number;
  description: string;
  available: boolean;
  reason?: string;
  summon?: string;
}
export interface PlayerView { id: PlayerId; name: string; mana: number | string; actions: number; ai?: string | null }
export interface LogEntry { id: string; turn: number; text: string }
export interface MatchView {
  replay?: ReplayView;
  sessionId: string;
  revision: string;
  preset: Preset;
  width: number;
  height: number;
  cells: CellView[];
  pieces: PieceView[];
  players: PlayerView[];
  currentPlayer: PlayerId;
  turn: number | string;
  hand: CardView[];
  onboard: CardView[];
  log: LogEntry[];
  lastAction?: { from?: Position; to?: Position };
  winner?: PlayerId | null;
  choices?: Choice[];
  config?: MatchConfig;
  aiPaused?: boolean;
  canUndo?: boolean;
  promotion?: boolean;
  over?: boolean;
  draw?: boolean;
  scheduled?: { id: string; text: string }[];
}

export interface Choice { id: string; kind: string; label: string; source?: string; card?: string; ability?: string; position?: Position | null; passenger?: Position }
export interface MatchConfig { preset: Preset; seats: string[] }
export interface Preview { revision: string; choiceId: string; label: string; lines: string[] }

export interface ReplayView {
  index: number; total: number;
  entries: {index:number; label:string; turn:string; player:number}[];
  ghost: null | {label:string; lines:string[]; marks:{position:Position;kind:string}[]; paths:{from:Position;to:Position}[]; tokens:{position:Position;archetype:string;owner:PlayerId}[]};
}
