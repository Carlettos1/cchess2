import { describe, expect, it } from 'vitest';
import { dimensions, toScreen, toWorld } from './geometry';
describe('camera coordinates', () => {
  it.each([0,1,2,3])('keeps every rectangular board cell and coordinate aligned at rotation %s', rotation => {
    const width=16,height=17,size=dimensions(width,height,rotation);
    const seen=new Set<string>();
    for(let x=0;x<width;x++) for(let y=0;y<height;y++) {
      const screen=toScreen({x,y},width,height,rotation);
      expect(screen.x).toBeGreaterThanOrEqual(0);expect(screen.x).toBeLessThan(size.width);
      expect(screen.y).toBeGreaterThanOrEqual(0);expect(screen.y).toBeLessThan(size.height);
      expect(toWorld(screen,width,height,rotation)).toEqual({x,y});seen.add(`${screen.x}:${screen.y}`);
    }
    expect(seen.size).toBe(width*height);
  });
});
