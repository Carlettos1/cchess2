import { useEffect, useMemo, useRef, useState } from 'react';
import { Focus, Home, Minus, Plus, RotateCw } from 'lucide-react';
import type { MatchView, PieceView, TargetView } from '../../bridge/types';
import { coordinate, dimensions, fileName, toScreen, toWorld } from './geometry';
interface Props { view:MatchView; selected?:string; targets:TargetView[]; onPiece:(p?:PieceView)=>void; onTarget:(t:TargetView)=>void }
const key=(p:{x:number;y:number})=>`${p.x}:${p.y}`;
export function Board({view,selected,targets,onPiece,onTarget}:Props){
  const [zoom,setZoom]=useState(1),[rotation,setRotation]=useState(0);
  const [space,setSpace]=useState({width:600,height:500});
  const viewport=useRef<HTMLDivElement>(null);
  const pan=useRef<{x:number;y:number;left:number;top:number} | undefined>(undefined);
  const pieceAt=useMemo(()=>new Map(view.pieces.map(p=>[key(p.position),p])),[view.pieces]);
  const targetAt=useMemo(()=>new Map(targets.map(t=>[key(t.position),t])),[targets]);
  const ghost=view.replay?.ghost;
  const point=(p:{x:number;y:number})=>toScreen(p,view.width,view.height,rotation);
  const size=dimensions(view.width,view.height,rotation);
  const unit=Math.max(4,Math.min((space.width-68)/size.width,(space.height-66)/size.height))*zoom;
  useEffect(()=>{const node=viewport.current;if(!node)return;const observer=new ResizeObserver(()=>setSpace({width:node.clientWidth,height:node.clientHeight}));observer.observe(node);setSpace({width:node.clientWidth,height:node.clientHeight});return()=>observer.disconnect();},[]);
  useEffect(()=>{setZoom(1);setRotation(0);},[view.preset]);
  const center=()=>{const node=viewport.current;const cell=node?.querySelector<HTMLElement>('.board-cell.selected');if(node&&cell){const n=node.getBoundingClientRect(),c=cell.getBoundingClientRect();node.scrollTo({left:node.scrollLeft+c.left-n.left-node.clientWidth/2+c.width/2,top:node.scrollTop+c.top-n.top-node.clientHeight/2+c.height/2,behavior:'auto'});}};
  return <section className="board-panel" aria-label="Game board"><div className="board-toolbar"><div className="legend">{view.replay && <span>Dashed = next action · × = removal · ! = damage</span>}<span><i className="move-mark"/>Move</span><span><i className="take-mark"/>Take / Attack</span></div><div className="camera-controls"><button aria-label="Zoom out" onClick={()=>setZoom(z=>Math.max(.7,z-.2))}><Minus/></button><span>{Math.round(zoom*100)}%</span><button aria-label="Zoom in" onClick={()=>setZoom(z=>Math.min(4,z+.2))}><Plus/></button><button aria-label="Fit board" onClick={()=>{setZoom(1);viewport.current?.scrollTo(0,0);}}><Home/></button><button aria-label="Rotate board" onClick={()=>setRotation(r=>(r+1)%4)}><RotateCw/></button><button disabled={!selected} aria-label="Center selected" onClick={center}><Focus/></button></div></div>
    <div className="board-viewport" ref={viewport} onPointerDown={e=>{if(e.target!==e.currentTarget)return;pan.current={x:e.clientX,y:e.clientY,left:e.currentTarget.scrollLeft,top:e.currentTarget.scrollTop};e.currentTarget.setPointerCapture(e.pointerId);}} onPointerMove={e=>{if(pan.current){e.currentTarget.scrollLeft=pan.current.left+pan.current.x-e.clientX;e.currentTarget.scrollTop=pan.current.top+pan.current.y-e.clientY;}}} onPointerUp={()=>{pan.current=undefined;}} onPointerCancel={()=>{pan.current=undefined;}}>
      <div className="board-stage" style={{width:unit*size.width,height:unit*size.height,flexShrink:0}}><div className="board-grid" style={{gridTemplateColumns:`repeat(${size.width},1fr)`,gridTemplateRows:`repeat(${size.height},1fr)`}}>
      {view.cells.map(cell=>{const p=pieceAt.get(key(cell.position)),target=targetAt.get(key(cell.position)),screen=toScreen(cell.position,view.width,view.height,rotation);return <button key={key(cell.position)} disabled={!cell.playable} style={{gridColumn:screen.x+1,gridRow:screen.y+1}} className={`board-cell ${(cell.position.x+cell.position.y)%2?'dark':'light'} ${cell.playable?'':'void'} ${p && selected===p.id?'selected':''}`} aria-label={`${coordinate(cell.position)}${p?`, Player ${p.owner} ${p.archetype}`:''}${target?`, legal ${target.kind} target`:''}`} onClick={()=>target?onTarget(target):onPiece(p)}>
        {cell.magic&&<span className="magic-glyph" aria-hidden="true">◇</span>}{target&&<span className={`target ${target.kind}`} aria-hidden="true"/>}
        {p&&<span className={`piece-token owner-${p.owner}`}><span className="piece-label" style={{fontSize:Math.max(8,Math.min(19,unit*.3))}}>{p.label}</span><span className="seat-marker">{['●','◆','■','▲'][p.owner]}</span>{p.statuses.length>0&&<span className="status-count">{p.statuses.length}</span>}{p.damage>0&&<span className="damage-count">−{p.damage}</span>}</span>}
      </button>;})}</div>
      {ghost && <svg className="ghost-overlay" viewBox={`0 0 ${size.width} ${size.height}`} role="img" aria-label={`Next action ghost: ${ghost.label}`}>
        {ghost.marks.map((m,i)=>{const p=point(m.position);return <g key={i}><rect className={`ghost-mark ghost-${m.kind}`} x={p.x+.06} y={p.y+.06} width={.88} height={.88} rx={.08}/>{(m.kind==='removed'||m.kind==='damage')&&<text x={p.x+.82} y={p.y+.3} fontSize=".3" fill="#fff">{m.kind==='removed'?'×':'!'}</text>}</g>;})}
        {ghost.paths.map((path,i)=>{const a=point(path.from),b=point(path.to);return <g key={i}><line className="ghost-path" x1={a.x+.5} y1={a.y+.5} x2={b.x+.5} y2={b.y+.5}/><circle cx={b.x+.5} cy={b.y+.5} r=".12" fill="#8af3ea"/></g>;})}
        {ghost.tokens.map((token,i)=>{const p=point(token.position);const label=view.pieces.find(piece=>piece.archetype===token.archetype)?.label??token.archetype.slice(0,2);return <g key={i} aria-label={`Ghost Player ${token.owner} ${token.archetype}`}><circle className="ghost-piece" cx={p.x+.5} cy={p.y+.5} r=".32"/><text x={p.x+.5} y={p.y+.56} textAnchor="middle" fontSize=".23" fill="#fff">{label}</text><text x={p.x+.5} y={p.y+.76} textAnchor="middle" fontSize=".13" fill="#fff">P{token.owner}</text></g>;})}
      </svg>}
      <div className="rank-labels" style={{gridTemplateRows:`repeat(${size.height},1fr)`}}>{Array.from({length:size.height},(_,y)=>{const p=toWorld({x:0,y},view.width,view.height,rotation);return <span key={y}>{rotation%2?fileName(p.x):p.y+1}</span>;})}</div>
      <div className="file-labels">{Array.from({length:size.width},(_,x)=>{const p=toWorld({x,y:size.height-1},view.width,view.height,rotation);return <span key={x}>{rotation%2?p.y+1:fileName(p.x)}</span>;})}</div>
      </div>
    </div>
  </section>;
}
