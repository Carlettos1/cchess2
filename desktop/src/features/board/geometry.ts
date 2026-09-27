import type { Position } from '../../bridge/types';
export function dimensions(width:number,height:number,rotation:number) { return rotation % 2 ? {width:height,height:width} : {width,height}; }
export function toScreen(p:Position,width:number,height:number,rotation:number):Position {
  switch(rotation){case 1:return{x:p.y,y:p.x};case 2:return{x:width-1-p.x,y:p.y};case 3:return{x:height-1-p.y,y:width-1-p.x};default:return{x:p.x,y:height-1-p.y};}
}
export function toWorld(p:Position,width:number,height:number,rotation:number):Position {
  switch(rotation){case 1:return{x:p.y,y:p.x};case 2:return{x:width-1-p.x,y:p.y};case 3:return{x:width-1-p.y,y:height-1-p.x};default:return{x:p.x,y:height-1-p.y};}
}
export function fileName(x:number) {let n=x+1,out='';while(n){n--;out=String.fromCharCode(97+n%26)+out;n=Math.floor(n/26);}return out;}
export function coordinate(p:Position){return `${fileName(p.x)}${p.y+1}`;}
