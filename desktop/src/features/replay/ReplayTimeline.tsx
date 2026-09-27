import {useEffect, useState} from 'react';
import type {ReplayView} from '../../bridge/types';
export function ReplayTimeline({replay,busy,onSeek,onExit}:{replay:ReplayView;busy:boolean;onSeek:(index:number)=>Promise<unknown>;onExit:()=>void}) {
  const [playing,setPlaying]=useState(false), [speed,setSpeed]=useState(1000);
  const [position,setPosition]=useState(replay.index);
  useEffect(()=>setPosition(replay.index),[replay.index]);
  useEffect(()=>{
    if (!playing || busy) return;
    if(replay.index>=replay.total){setPlaying(false);return;}
    const timer=setTimeout(()=>void onSeek(replay.index+1),speed);
    return()=>clearTimeout(timer);
  },[playing,busy,replay.index,replay.total,speed,onSeek]);
  const seek=(index:number)=>{setPlaying(false);setPosition(index);void onSeek(index);};
  return <section className="replay-timeline" aria-label="Replay timeline" onKeyDown={e=>{
    if((e.target as HTMLElement).matches('input,select'))return;
    const index=e.key==='ArrowLeft'?replay.index-1:e.key==='ArrowRight'?replay.index+1:e.key==='Home'?0:e.key==='End'?replay.total:undefined;
    if(index!==undefined){e.preventDefault();seek(Math.max(0,Math.min(replay.total,index)));}
  }}>
    <div className="replay-controls"><strong>REPLAY · {replay.index} / {replay.total}</strong>
      <button disabled={busy||replay.index===0} onClick={()=>seek(0)} aria-label="First position">|◀</button>
      <button disabled={busy||replay.index===0} onClick={()=>seek(replay.index-1)} aria-label="Previous action">◀</button>
      <button disabled={replay.total===0||replay.index===replay.total} onClick={()=>setPlaying(p=>!p)}>{playing?'Pause replay':'Play replay'}</button>
      <button disabled={busy||replay.index===replay.total} onClick={()=>seek(replay.index+1)} aria-label="Next action">▶</button>
      <button disabled={busy||replay.index===replay.total} onClick={()=>seek(replay.total)} aria-label="Final position">▶|</button>
      <select aria-label="Replay speed" value={speed} onChange={e=>setSpeed(Number(e.target.value))}><option value={2000}>0.5×</option><option value={1000}>1×</option><option value={500}>2×</option></select>
      <button disabled={busy} onClick={()=>{setPlaying(false);onExit();}}>Exit replay</button>
    </div>
    <input aria-label="Replay position" type="range" min={0} max={replay.total} value={position} onChange={e=>seek(Number(e.target.value))}/>
    <div className="replay-entries">{replay.entries.map(entry=><button key={entry.index} aria-current={entry.index===replay.index?'step':undefined} onClick={()=>seek(entry.index)} title={`Before action ${entry.index+1} · Turn ${entry.turn}`}><small>{entry.index+1} · P{entry.player}</small>{entry.label}</button>)}{replay.index===replay.total&&<span>End of replay</span>}{replay.total===0&&<span>Play some actions, or load a saved match to watch its replay.</span>}</div>
  </section>;
}
