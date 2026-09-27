import type { MatchView } from './types';
import { invoke, isTauri } from '@tauri-apps/api/core';

export async function request<T = MatchView>(payload: Record<string, unknown>): Promise<T> {
  if (isTauri()) return invoke<T>('game_request', { request: payload });
  const response = await fetch('/api/game', { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(payload) });
  if (!response.ok) throw new Error(await response.text());
  return response.json() as Promise<T>;
}
