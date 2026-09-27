import { LockKeyhole } from "lucide-react";
import type { CardView } from "../../bridge/types";

export function Hand({ cards, onChoose }: { cards: CardView[]; onChoose: (card: CardView) => void }) {
  return <section className="hand-panel" aria-label="Your hand"><div className="hand-title"><small>YOUR HAND</small><b>{cards.length} cards</b></div><div className="cards-strip">{cards.map((card) => <button key={card.id} className={`card ${card.kind} ${!card.available ? "unavailable" : ""}`} onClick={() => onChoose(card)} aria-label={`${card.name}, ${card.cost} mana${card.reason ? `, unavailable: ${card.reason}` : ""}`}>
    <img src={`/cards/${card.kind}.svg`} alt=""/><span className="card-cost">{card.cost}</span><span className="card-type">{card.kind.toUpperCase()}</span><b>{card.name}</b><p>{card.description}</p>{!card.available && <span className="card-reason"><LockKeyhole/>{card.reason}</span>}
  </button>)}</div></section>;
}
