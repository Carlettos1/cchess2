import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { Board } from './Board';
import { makeDemoMatch } from '../../bridge/fixture';
beforeEach(()=>{vi.stubGlobal('ResizeObserver',class { observe(){} disconnect(){} });});
afterEach(()=>{cleanup();vi.unstubAllGlobals();});
it('has no selected empty cells and preserves unique labels on the largest board',()=>{
 const view=makeDemoMatch('cross24');const {container,rerender}=render(<Board view={view} targets={[]} onPiece={()=>{}} onTarget={()=>{}}/>);
 expect(container.querySelectorAll('.board-cell.selected')).toHaveLength(0);
 expect(screen.getAllByText('Kn').length).toBeGreaterThan(0);
 expect(container.querySelectorAll('.piece-token img')).toHaveLength(0);
 rerender(<Board view={view} selected={view.pieces[0].id} targets={[]} onPiece={()=>{}} onTarget={()=>{}}/>);
 expect(container.querySelectorAll('.board-cell.selected')).toHaveLength(1);
});
it('rotates cells and coordinate labels together without changing the clicked world piece',()=>{
 const view=makeDemoMatch('classic');const onPiece=vi.fn();const {container}=render(<Board view={view} targets={[]} onPiece={onPiece} onTarget={()=>{}}/>);
 fireEvent.click(screen.getByRole('button',{name:'Rotate board'}));
 const a1=screen.getByRole('button',{name:'a1, Player 0 rook'});
 expect(a1.style.gridRow).toBe('1');expect(a1.style.gridColumn).toBe('1');
 expect(container.querySelector('.rank-labels')?.textContent).toBe('abcdefgh');
 expect(container.querySelector('.file-labels')?.textContent).toBe('12345678');
 fireEvent.click(a1);expect(onPiece).toHaveBeenCalledWith(expect.objectContaining({position:{x:0,y:0}}));
});
it('shows the next move as a separate rotated ghost without moving the real piece',()=>{
 const view=makeDemoMatch('classic');view.replay={index:0,total:1,entries:[],ghost:{label:'Knight leap',lines:[],marks:[{position:{x:1,y:0},kind:'source'}],paths:[{from:{x:1,y:0},to:{x:0,y:2}}],tokens:[{position:{x:0,y:2},archetype:'knight',owner:0}]}};
 const {container}=render(<Board view={view} targets={[]} onPiece={()=>{}} onTarget={()=>{}}/>);
 expect(screen.getByRole('button',{name:'b1, Player 0 knight'})).toBeInTheDocument();
 expect(screen.getByRole('img',{name:'Next action ghost: Knight leap'})).toBeInTheDocument();
 fireEvent.click(screen.getByRole('button',{name:'Rotate board'}));
 expect(container.querySelector('.ghost-path')).toHaveAttribute('x2','2.5');
 expect(container.querySelector('.ghost-path')).toHaveAttribute('y2','0.5');
});
