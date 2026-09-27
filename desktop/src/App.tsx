import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { BookOpen, Clock3, History, Pause, Play, RotateCcw, Save, Settings, X, Upload } from 'lucide-react';
import { request } from './bridge';
import type { CardView, Choice, MatchConfig, MatchView, PieceView, Preset, Preview, TargetView } from './bridge/types';
import { Board } from './features/board/Board';
import { Inspector } from './features/inspector/Inspector';
import { ReplayTimeline } from './features/replay/ReplayTimeline';
import { Hand } from './features/cards/Hand';

export function App() {
  const [view, setView] = useState<MatchView>();
  const [selected, setSelected] = useState<string>();
  const [card, setCard] = useState<string>();
  const [ability, setAbility] = useState<string>();
  const [passenger, setPassenger] = useState<{x:number;y:number}>();
  const [preview, setPreview] = useState<Preview>();
  const [notice, setNotice] = useState('Select a piece or card. All actions are validated by the Rust engine.');
  const [busy, setBusy] = useState(false);
  const [thinking, setThinking] = useState(false);
  const aiRunning = useRef(false);
  const seekTarget = useRef<number | undefined>(undefined);
  const seeking = useRef(false);
  const [searchEpoch, setSearchEpoch] = useState(0);
  const [drawer, setDrawer] = useState<'history' | 'schedule' | 'rules' | 'onboard' | null>(null);
  const [setup, setSetup] = useState(false);
  const [settings, setSettings] = useState(false);
  const [confirmMoves, setConfirmMoves] = useState(true);
  const [handoff, setHandoff] = useState(false);
  const fileInput = useRef<HTMLInputElement>(null);
  const currentView = useRef<MatchView | undefined>(undefined);
  const clearSelection = useCallback(() => { setSelected(undefined); setCard(undefined); setAbility(undefined); setPassenger(undefined); setPreview(undefined); }, []);
  const accept = useCallback((next: MatchView) => {
    const old = currentView.current;
    if (old && BigInt(next.revision) < BigInt(old.revision)) return;
    if (!old || old.revision !== next.revision) {
      clearSelection();
      if (!next.replay && !old?.replay && (!old || old.currentPlayer !== next.currentPlayer) && next.config?.seats.filter(s => s === 'human').length! > 1 && !next.players.find(p => p.id === next.currentPlayer)?.ai && !next.over) setHandoff(true);
    }
    if(next.replay) setHandoff(false);
    currentView.current = next; setView(next);
  }, [clearSelection]);
  const run = useCallback(async (op: string, args: Record<string, unknown> = {}) => {
    setBusy(true);
    try { const next = await request({ op, revision: currentView.current?.revision, ...args }); accept(next); setNotice(next.promotion ? 'Choose a promotion before continuing.' : next.over ? next.draw ? 'Draw.' : `Player ${next.winner} wins.` : 'Ready. Select a piece or card.'); }
    catch (error) { setNotice(String(error)); if (op !== 'view') { try { accept(await request({op:'view'})); } catch { /* Keep the original error visible. */ } } return false; }
    finally { setBusy(false); }
    return true;
  }, [accept]);
  useEffect(() => { void run('view'); }, [run]);
  const current = view?.players.find(p => p.id === view.currentPlayer);
  useEffect(() => {
    if (aiRunning.current || !view || view.replay || !current?.ai || view.aiPaused || view.over || busy || setup || settings) return;
    let alive = true;
    const timer = setTimeout(async () => {
      aiRunning.current = true; setThinking(true);
      try { const next = await request({ op: 'ai', revision: view.revision }); if (alive) accept(next); }
      catch (error) { if (alive) { setNotice(String(error)); void run('pause', { paused: true }); } }
      finally { aiRunning.current = false; setThinking(false); setSearchEpoch(n => n + 1); }
    }, 350);
    return () => { alive = false; clearTimeout(timer); setThinking(false); };
  }, [view?.revision, view?.aiPaused, view?.over, current?.ai, busy, setup, settings, accept, run, searchEpoch]);
  useEffect(() => {
    const key = (event: KeyboardEvent) => { if (event.key === 'Escape' && !busy) { if (setup) setSetup(false); else if (settings) setSettings(false); else if (drawer) setDrawer(null); else if (!view?.promotion) { clearSelection(); setNotice('Selection cancelled.'); } } };
    window.addEventListener('keydown', key); return () => window.removeEventListener('keydown', key);
  }, [busy, setup, settings, drawer, view?.promotion, clearSelection]);

  const piece = view?.pieces.find(p => p.id === selected);
  const choices = useMemo(() => (view?.choices ?? []).filter(c => view?.promotion ? c.kind === 'promotion' : card ? c.card === card : selected ? c.source === selected && (!ability || c.ability === ability) : false), [view, selected, card, ability]);
  const choosingPassenger = !!ability && !passenger && choices.some(c => c.passenger);
  const stagedChoices = passenger ? choices.filter(c => c.passenger?.x === passenger.x && c.passenger?.y === passenger.y) : choices;
  const targets: TargetView[] = stagedChoices.filter(c => choosingPassenger ? c.passenger : c.position).map(c => ({ position: choosingPassenger ? c.passenger! : c.position!, kind: c.kind === 'move' || c.kind === 'take' || c.kind === 'attack' ? c.kind : 'ability', choiceId: c.id }));
  const selectPiece = (next?: PieceView) => { if (busy || (view?.promotion && !view.replay)) return; clearSelection(); setSelected(next?.id); setNotice(next ? next.owner === view?.currentPlayer ? 'Choose an action below or a highlighted square.' : 'Other player’s piece: inspect only.' : 'Select a piece or card.'); };
  const choose = async (choice: Choice) => {
    if (!view || view.replay || busy || current?.ai || handoff) return;
    if (!confirmMoves && choice.kind === 'move') { await run('submit', { choiceId: choice.id }); return; }
    setBusy(true);
    try { const result = await request<Preview>({ op: 'preview', revision: view.revision, choiceId: choice.id }); if (currentView.current?.revision === result.revision) { setPreview(result); setNotice(result.label + '\n' + result.lines.join('\n')); } }
    catch (error) { setNotice(String(error)); }
    finally { setBusy(false); }
  };
  const chooseTarget = (target: TargetView) => {
    if (choosingPassenger) { setPassenger(target.position); setNotice('Passenger selected. Choose a highlighted destination.'); return; }
    const same = stagedChoices.filter(c => c.position?.x === target.position.x && c.position?.y === target.position.y);
    if (same.length === 1) void choose(same[0]); else { setPreview(undefined); setNotice('Several actions share this square. Choose the exact action in the action list.'); }
  };
  const chooseCard = (next: CardView) => { if (busy || view?.promotion) return; clearSelection(); setCard(next.id); setNotice(next.available ? `${next.name}: choose a legal action below.` : `${next.name}: ${next.reason}`); };
  const save = async () => {
    try { await request({op:'save_local'}); setNotice('Match saved locally. Load restores this save.'); }
    catch (error) { setNotice(String(error)); }
  };
  const openSetup = async () => { await run('pause', { paused: true }); setSetup(true); };
  const seekReplay = useCallback(async (index: number) => {
    seekTarget.current = index;
    if(seeking.current) return;
    seeking.current = true; setBusy(true);
    try {
      while(seekTarget.current !== undefined) {
        const target = seekTarget.current; seekTarget.current = undefined;
        const next = await request({op:'replay_seek',index:target,revision:currentView.current?.revision});
        accept(next);
      }
    } catch(error) {seekTarget.current=undefined;setNotice(String(error));}
    finally {seeking.current=false;setBusy(false);}
  },[accept]);
  if (!view) return <main className="loading"><p role="status">{busy ? 'Starting the Rust engine…' : notice}</p>{!busy && <button onClick={() => void run('view')}>Retry</button>}</main>;
  const locked = !!view.replay || busy || handoff || !!current?.ai || !!view.over;
  const end = view.choices?.find(c => c.kind === 'end');
  return <main className="app-shell">
    <header className="brand-bar"><button disabled={!!view.replay} className="wordmark" onClick={() => void openSetup()}>CCHESS</button><span className="edition">ARCANE TABLE · LIVE ENGINE</span><span className="spacer"/><button className="text-button" disabled={busy || !!view.replay} onClick={() => void openSetup()}>New match</button><button className="icon-button" aria-label="Settings" disabled={busy || !!view.replay} onClick={() => {void run('pause', {paused:true}).then(() => setSettings(true));}}><Settings size={18}/></button></header>
    <section className="turn-bar" aria-label="Match status">
      <div className="turn-owner"><span className={`seat-dot p${current?.id}`}>{['●','◆','■','▲'][view.currentPlayer]}</span><strong>P{view.currentPlayer} · {view.replay ? 'REPLAY' : view.over ? 'GAME OVER' : current?.ai ? 'AI TURN' : 'YOUR TURN'}</strong><small>Turn {view.turn}</small></div>
      <div className="resource"><small>Mana</small><b>{current?.mana}</b></div><div className="resource"><small>Actions</small><b>{current?.actions}</b></div>
      <div className="ai-state">{thinking ? 'Thinking…' : view.aiPaused ? 'AI paused' : current?.ai ?? 'Human'}</div>
      <button className="icon-button" disabled={busy || !!view.replay} aria-label={view.aiPaused ? 'Resume AI' : 'Pause AI'} onClick={() => void run('pause', { paused: !view.aiPaused })}>{view.aiPaused ? <Play/> : <Pause/>}</button>
      <button className="primary end-turn" disabled={locked || !end} onClick={() => end && void choose(end)}>End turn</button>
    </section>
    <section className="workspace">
      <Board view={view} selected={selected} targets={locked ? [] : targets} onPiece={selectPiece} onTarget={chooseTarget}/>
      <Inspector piece={piece} mode={preview ? 'preview' : busy ? 'submitting' : 'piece'} notice={view.replay ? view.replay.ghost ? `NEXT: ${view.replay.ghost.label}\n${view.replay.ghost.lines.join('\n')}` : 'End of replay. Return to any position using the timeline.' : notice} onCancel={() => { setPreview(undefined); setNotice('Preview cancelled.'); }} onConfirm={() => preview && void run('submit', { choiceId: preview.choiceId })} selectionTitle={view.replay ? view.replay.ghost?.label ?? "Replay complete" : view.promotion ? "Choose promotion" : view.over ? "Match complete" : view.hand.find(c => c.id === card)?.name} selectionDescription={view.replay ? "Solid pieces show the current position. Dashed ghosts show the next action. Drag the timeline or step through actions below." : view.hand.find(c => c.id === card)?.description} busy={busy || handoff || !!view.replay} onAbility={id => {setAbility(id);setPassenger(undefined);setPreview(undefined);setNotice(choices.some(c => c.ability === id && c.passenger) ? 'Choose a highlighted passenger, then its destination.' : 'Choose a target or an action below.');}}>
        {view.over && <p role="status">{view.draw ? 'Draw' : `Player ${view.winner} wins`}</p>}
        {!view.replay && view.promotion && <h2>Promotion required</h2>}
        {!view.replay && (selected || card || view.promotion) && <section className="legal-actions" aria-label="Legal actions"><h2>{ability ? 'Ability targets' : card ? 'Card actions' : 'Legal actions'}</h2>{ability && <button onClick={() => {setAbility(undefined);setPassenger(undefined);}}>All piece actions</button>}{stagedChoices.length ? stagedChoices.map(c => <button key={c.id} disabled={locked} onClick={() => void choose(c)}>{c.label}</button>) : <p className="muted">No legal actions: check the turn, resources, acted state, cooldowns and targets.</p>}</section>}
      </Inspector>
    </section>
    {view.replay ? <ReplayTimeline replay={view.replay} busy={busy} onSeek={seekReplay} onExit={()=>void run('replay_exit')}/> : <Hand cards={handoff ? [] : view.hand} onChoose={chooseCard}/>}
    <nav className="tool-dock" aria-label="Match tools">
      <button disabled={!!view.replay || busy || !view.canUndo || handoff} onClick={() => void run('undo')}><RotateCcw size={17}/>Undo</button>
      <button disabled={busy || !!view.replay} onClick={() => void save()}><Save size={17}/>Save</button>
      <button disabled={busy || !!view.replay} onClick={() => void run('load_local')}><Upload size={17}/>Load</button>
      <button disabled={busy || !!view.replay} onClick={()=>void run('replay_start')}><Play size={17}/>Watch replay</button>
      <button onClick={() => setDrawer(drawer === 'history' ? null : 'history')}><History size={17}/>History</button>
      <button onClick={() => setDrawer(drawer === 'schedule' ? null : 'schedule')}><Clock3 size={17}/>Scheduled</button>
      <button onClick={() => setDrawer(drawer === 'onboard' ? null : 'onboard')}><BookOpen size={17}/>On-board</button>
      <button onClick={() => setDrawer(drawer === 'rules' ? null : 'rules')}><BookOpen size={17}/>Rules</button>
    </nav>
    <input ref={fileInput} type="file" accept=".json" hidden onChange={async e => { const file = e.target.files?.[0]; e.target.value = ''; if (!file) return; if (file.size > 8_000_000) { setNotice('Save is too large.'); return; } try { await run('load', { save: JSON.parse(await file.text()) }); } catch (error) { setNotice(String(error)); } }}/>
    {drawer && <aside className="drawer" aria-label={drawer}><div className="drawer-head"><h2>{drawer}</h2><button className="icon-button" aria-label="Close drawer" onClick={() => setDrawer(null)}><X/></button></div>
      {drawer === 'history' && <ol className="timeline">{view.log.length ? view.log.map(l => <li key={l.id}>{l.text}</li>) : <li>No actions yet.</li>}</ol>}
      {drawer === 'schedule' && <ul>{view.scheduled?.length ? view.scheduled.map(s => <li key={s.id}>{s.text}</li>) : <li>No scheduled work.</li>}</ul>}
      {drawer === 'onboard' && <ul>{view.onboard.length ? view.onboard.map(c => <li key={c.id}>{c.name} · card {c.id}</li>) : <li>No on-board cards.</li>}</ul>}
      {drawer === 'rules' && <div className="rule-grid"><article><b>Victory</b><p>Remove the opponent’s last Royal piece. There is no check or checkmate.</p></article><article><b>Move / Take / Attack</b><p>Move relocates. Take damages, then relocates only if the defender is removed. Attack deals damage without relocation.</p></article><article><b>Actions and clocks</b><p>Costs and legal targets come from the engine. Exhausting actions advances the turn. Owner-turn timers advance only on their owner’s turns.</p></article><article><b>Complex actions</b><p>Choose the exact action in the inspector. Transport lists passenger and destination; scheduled abilities list event IDs. Promotion must be resolved before continuing.</p></article></div>}
    </aside>}
    {setup && <SetupDialog config={view.config!} busy={busy} onClose={() => setSetup(false)} onStart={async config => { setHandoff(false); if (await run('new', { config })) setSetup(false); }}/>} 
    {settings && <div className="modal-backdrop"><section className="setup-modal" role="dialog" aria-modal="true" aria-label="Settings"><h2>Settings</h2><label><input type="checkbox" checked={confirmMoves} onChange={e => setConfirmMoves(e.target.checked)}/> Preview ordinary moves before committing</label><button className="secondary" onClick={() => {fileInput.current?.click();setSettings(false);}}>Import save file</button><p>Abilities, captures and cards always require confirmation. Reduced motion follows your system preference.</p><button className="primary" onClick={() => setSettings(false)}>Done</button></section></div>}
    {handoff && <div className="modal-backdrop"><section className="setup-modal" role="dialog" aria-modal="true" aria-label="Pass the game"><h2>Pass to Player {view.currentPlayer}</h2><p>The incoming player can reveal their hand when ready.</p><button className="primary" onClick={() => setHandoff(false)}>Reveal my hand</button></section></div>}
  </main>;
}
function SetupDialog({config,busy,onClose,onStart}:{config:MatchConfig;busy:boolean;onClose:()=>void;onStart:(c:MatchConfig)=>void}) {
  const [preset,setPreset]=useState<Preset>(config.preset);
  const [seats,setSeats]=useState(config.seats);
  const options: [Preset,string][]=[['classic','Classical · 8×8'],['expansion','Full armies · 16×17'],['cross14','Cross · 14×14'],['cross24','Full cross · 24×24'],['abilities','Abilities demo'],['cards','Cards demo'],['siege','Siege demo'],['events','Events demo'],['catalogue','Catalogue demo']];
  return <div className="modal-backdrop"><section className="setup-modal" role="dialog" aria-modal="true" aria-label="New match"><h1>New local match</h1><p>Starting a match replaces the current position. Save first to keep it.</p><label>Board <select value={preset} onChange={e=>{const p=e.target.value as Preset;setPreset(p);const n=p.startsWith('cross')?4:2;setSeats(Array.from({length:n},(_,i)=>seats[i]??'human'));}}>{options.map(([id,name])=><option key={id} value={id}>{name}</option>)}</select></label><div className="seat-row">{seats.map((s,i)=><label key={i}>P{i} <select value={s} onChange={e=>setSeats(seats.map((old,j)=>j===i?e.target.value:old))}>{['human','random','first','heuristic','mcts'].map(p=><option key={p}>{p}</option>)}</select></label>)}</div><button className="primary" disabled={busy} onClick={()=>onStart({preset,seats})}>Start match</button><button className="secondary" disabled={busy} onClick={onClose}>Cancel</button></section></div>;
}
