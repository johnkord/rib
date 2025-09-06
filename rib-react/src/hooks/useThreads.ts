import { useQuery, useQueryClient } from '@tanstack/react-query';
import { fetchJson, postJson, uploadImage } from '../lib/api';

export interface Thread {
  id: number;
  subject: string;
  body: string;            // NEW
  board_id: number;
  reply_count: number;
  created_at: string;        // NEW
  bump_time:  string;        // NEW
  image_hash?: string;
  mime?: string;
  deleted_at?: string | null;
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

    await postJson('/threads', { board_id: boardId, subject, body, image_hash, mime }); // body added
    await qc.invalidateQueries({ queryKey: ['threads', boardId] });
  };
}
