import { useQuery, useQueryClient } from '@tanstack/react-query';
import { fetchJson, postJson, uploadImage } from '../lib/api';

export interface Thread {
  id: number;
  subject: string;
  body: string;
  board_id: number;
  reply_count: number;
  created_at: string;
  bump_time:  string;
  image_hash?: string;
  mime?: string;
  deleted_at?: string | null;
  created_by: string; // Added created_by field
}

export function useThreads(boardId: number | null, includeDeleted: boolean) {
  return useQuery<Thread[]>({
    queryKey: ['threads', boardId, includeDeleted],
    queryFn: () => fetchJson(`/boards/${boardId}/threads${includeDeleted ? '?include_deleted=1' : ''}`),
    enabled: !!boardId,
  });
}

export function useCreateThread() {
  const qc = useQueryClient();
  return async (boardId: number, subject: string, body: string, file?: File | null) => {
    let image_hash: string | undefined;
    let mime: string | undefined;

    if (file) {
      const up = await uploadImage(file);
      image_hash = up.hash;
      mime = up.mime;
    }

  await postJson('/threads', { board_id: boardId, subject, body, image_hash, mime, created_by: '' }); // Added created_by
    await qc.invalidateQueries({ queryKey: ['threads', boardId] });
  };
}
