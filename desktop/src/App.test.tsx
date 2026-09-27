import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { App } from './App';
import { request } from './bridge';
import { makeDemoMatch } from './bridge/fixture';
import type { MatchView } from './bridge/types';
vi.mock('./bridge',()=>({request:vi.fn()}));
const rpc=vi.mocked(request);
beforeEach(()=>{rpc.mockReset();vi.stubGlobal('ResizeObserver',class{observe(){}disconnect(){}});});
afterEach(()=>{cleanup();vi.unstubAllGlobals();});
function initial():MatchView {return {...makeDemoMatch('classic'),revision:'41',config:{preset:'classic',seats:['human','heuristic']},aiPaused:true,choices:[],promotion:false};}
it('previews without moving and submits the typed choice rather than parsing presentation text',async()=>{
 const view=initial();const knight=view.pieces.find(p=>p.owner===0&&p.archetype==='knight')!;
 view.choices=[{id:'41:0',source:knight.id,kind:'move',label:'Knight leap',position:{x:0,y:2}}];
 const next={...view,revision:'42',turn:13,currentPlayer:1,pieces:view.pieces.map(p=>p.id===knight.id?{...p,position:{x:0,y:2}}:p),choices:[]} as MatchView;
 rpc.mockImplementation(async payload=>{if(payload.op==='view')return view;if(payload.op==='preview')return {revision:'41',choiceId:'41:0',label:'Translated preview with no coordinate arrow',lines:['The turn will advance.']};if(payload.op==='submit')return next;throw Error('Unexpected request');});
 render(<App/>);await screen.findByRole('button',{name:'b1, Player 0 knight'});
 fireEvent.click(screen.getByRole('button',{name:'b1, Player 0 knight'}));fireEvent.click(screen.getByRole('button',{name:'a3, legal move target'}));
 await screen.findByRole('button',{name:'Confirm'});expect(screen.getByRole('button',{name:'b1, Player 0 knight'})).toBeInTheDocument();
 fireEvent.click(screen.getByRole('button',{name:'Confirm'}));
 await screen.findByRole('button',{name:'a3, Player 0 knight'});
 expect(rpc).toHaveBeenCalledWith({op:'submit',revision:'41',choiceId:'41:0'});
 expect(screen.getByText('Turn 13')).toBeInTheDocument();
});
it('requires the transport passenger before selecting its destination on the board',async()=>{
 const view=initial();const caster=view.pieces.find(p=>p.owner===0&&p.archetype==='knight')!;
 caster.archetype='catapult';caster.abilities=[{id:'transport',name:'Transport',description:'Choose passenger',actions:1,mana:0,available:true}];
 view.choices=[{id:'41:1',source:caster.id,kind:'ability',ability:'transport',passenger:{x:1,y:1},position:{x:1,y:2},label:'Transport b2 to b3'}];
 rpc.mockImplementation(async payload=>payload.op==='view'?view:{revision:'41',choiceId:'41:1',label:'Transport',lines:['Passenger moves']});
 render(<App/>);fireEvent.click(await screen.findByRole('button',{name:'b1, Player 0 catapult'}));
 fireEvent.click(within(screen.getByLabelText('Inspector')).getByRole('button',{name:/Transport 1 action/}));
 fireEvent.click(screen.getByRole('button',{name:'b2, Player 0 pawn, legal ability target'}));
 expect(rpc).toHaveBeenCalledTimes(1);
 fireEvent.click(screen.getByRole('button',{name:'b3, legal ability target'}));
 await waitFor(()=>expect(rpc).toHaveBeenCalledWith({op:'preview',revision:'41',choiceId:'41:1'}));
});
it('makes mandatory promotion choices directly visible and blocks end turn',async()=>{
 const view=initial();view.promotion=true;view.choices=[{id:'41:3',kind:'promotion',label:'Promote to queen'}];rpc.mockResolvedValue(view);
 render(<App/>);expect(await screen.findByRole('heading',{name:'Choose promotion'})).toBeInTheDocument();
 expect(screen.getByRole('button',{name:'Promote to queen'})).toBeEnabled();expect(screen.getByRole('button',{name:'End turn'})).toBeDisabled();
 expect(screen.queryByRole('heading',{name:'Explore the table'})).not.toBeInTheDocument();
});
it('opens a read-only replay, seeks, and restores the live match',async()=>{
 const live=initial();const replay={...live,revision:'42',replay:{index:0,total:1,entries:[{index:0,label:'Knight leap',turn:'1',player:0}],ghost:{label:'Knight leap',lines:['Moves to a3'],marks:[],paths:[],tokens:[]}}};
 rpc.mockImplementation(async payload=>payload.op==='replay_start'?replay:payload.op==='replay_seek'?{...replay,revision:'43',replay:{...replay.replay,index:1,ghost:null}}: {...live,revision:payload.op==='replay_exit'?'44':'41'});
 render(<App/>);fireEvent.click(await screen.findByRole('button',{name:'Watch replay'}));
 await screen.findByRole('region',{name:'Replay timeline'});
 expect(screen.getByRole('button',{name:'End turn'})).toBeDisabled();expect(screen.getByRole('button',{name:'Save'})).toBeDisabled();
 fireEvent.change(screen.getByRole('slider',{name:'Replay position'}),{target:{value:'1'}});
 await screen.findByText('End of replay');expect(screen.getByRole('button',{name:'Next action'})).toBeDisabled();
 fireEvent.click(screen.getByRole('button',{name:'Exit replay'}));
 await waitFor(()=>expect(screen.queryByRole('slider')).not.toBeInTheDocument());
 expect(screen.getByRole('button',{name:'Save'})).toBeEnabled();
});
