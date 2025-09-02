import { useQuery, useQueryClient } from '@tanstack/react-query';
import { fetchJson, postJson, patchJson } from '../lib/api';

export interface Board { id: number; slug: string; title: string; }

export function useBoards() {
  return useQuery<Board[]>({ queryKey: ['boards'], queryFn: () => fetchJson('/boards') });
}

export function useCreateBoard() {
  const qc = useQueryClient();
  return async (slug: string, title: string) => {
    await postJson('/boards', { slug, title });
    await qc.invalidateQueries({ queryKey: ['boards'] });
  };
}

export function useUpdateBoard() {
  const qc = useQueryClient();
  return async (id: number, slug?: string, title?: string) => {
    await patchJson(`/boards/${id}`, { slug, title });
    await qc.invalidateQueries({ queryKey: ['boards'] });
  };
}
