import type { ReactNode } from "react";
import { AlertCircle, Check, ChevronRight, Heart, Shield, Sparkles, Tag, X } from "lucide-react";
import type { PieceView, UiMode } from "../../bridge/types";

export function Inspector({ piece, mode, notice, onCancel, onConfirm, onAbility, busy, children, selectionTitle, selectionDescription }: { piece?: PieceView; mode: UiMode; notice: string; onCancel: () => void; onConfirm: () => void; onAbility?: (id: string) => void; busy?: boolean; children?: ReactNode; selectionTitle?: string; selectionDescription?: string }) {
  return <aside className="inspector" aria-label="Inspector">
    <div className="inspector-scroll">
      <small className="eyebrow">{piece ? "SELECTED PIECE" : "INSPECTOR"}</small>
      {piece ? <>
        <div className="piece-heading"><div><h1>{title(piece.archetype)}</h1><p className={`owner-text owner-${piece.owner}`}>P{piece.owner} · {coordinate(piece.position)} · {piece.acted ? "Acted" : "Ready"}</p></div><div className={`portrait owner-${piece.owner}`}><img src={`/pieces/${piece.archetype}.svg`} alt=""/></div></div>
        <div className="stat-row"><span><Heart/>Health <b>{Math.max(0, piece.tough - piece.damage)} / {piece.tough}</b></span><span><Shield/>Damage <b>{piece.damage}</b></span></div>
        <div className="chips">{piece.tags.map((tag) => <span key={tag}><Tag/>{tag}</span>)}</div>
        {piece.statuses.length > 0 && <section className="status-list"><h2>Status</h2>{piece.statuses.map((s) => <div key={s.kind}><span className="status-icon">{s.kind[0]}</span><b>{s.kind}</b><span>{s.remaining} owner turns</span></div>)}</section>}
        <section className="ability-list"><h2>Abilities</h2>{piece.abilities.length ? piece.abilities.map((ability) => <button key={ability.id} disabled={busy || !ability.available} onClick={() => onAbility?.(ability.id)}><div><b>{ability.name}</b><ChevronRight/></div><em>{ability.actions} action · {ability.mana} mana</em><p>{ability.description}</p>{(ability.cooldown ?? 0) > 0 && <small>Cooldown: {ability.cooldown} owner turns</small>}{ability.reason && <small className="unavailable"><AlertCircle/>{ability.reason}</small>}</button>) : <p className="muted">This piece has no activated abilities.</p>}</section>
      </> : selectionTitle ? <div><h1>{selectionTitle}</h1><p>{selectionDescription}</p></div> : <div className="empty-inspector"><Sparkles/><h1>Explore the table</h1><p>Select a piece, a highlighted square, or a card. Rules details and availability reasons will appear here.</p></div>}
      {children}
    </div>
    <div role="status" aria-live="polite" className={`notice ${mode === "preview" ? "preview" : ""}`}><small>{mode === "preview" ? "ACTION PREVIEW" : "TARGETING"}</small><p>{notice}</p>{mode === "preview" && <div className="confirm-row"><button className="secondary" onClick={onCancel}><X/>Cancel</button><button className="primary" disabled={busy} onClick={onConfirm}><Check/>Confirm</button></div>}</div>
  </aside>;
}
function title(value: string) { return value.split("_").map((p) => p[0].toUpperCase() + p.slice(1)).join(" "); }
function coordinate(p: { x: number; y: number }) { return `${String.fromCharCode(97 + p.x)}${p.y + 1}`; }
