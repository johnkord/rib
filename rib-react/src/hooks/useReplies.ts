import { useQuery, useQueryClient } from '@tanstack/react-query';
import { fetchJson, postJson, uploadImage } from '../lib/api';

export interface Reply {
	id: number;
	thread_id: number;
	content: string;
	image_hash?: string;    // ...unchanged...
	mime?: string;          // ...unchanged...
  created_at: string;     // ISO timestamp
  deleted_at?: string | null;
  created_by: string; // author attribution
}

export function useReplies(threadId: number | null, includeDeleted: boolean) {
  return useQuery<Reply[]>({
    queryKey: ['replies', threadId, includeDeleted],
    queryFn: () => fetchJson(`/threads/${threadId}/replies${includeDeleted ? '?include_deleted=1' : ''}`),
    enabled: !!threadId,
  });
}

export function useCreateReply() {
  const qc = useQueryClient();
  return async (threadId: number, content: string, file?: File | null) => {
    let image_hash: string | undefined;
    let mime: string | undefined;

    if (file) {
      const uploaded = await uploadImage(file);   // { hash, mime, size }
      image_hash = uploaded.hash;
      mime = uploaded.mime;
    }

  await postJson('/replies', { thread_id: threadId, content, image_hash, mime, created_by: '' });
    await qc.invalidateQueries({ queryKey: ['replies', threadId] });
    await qc.invalidateQueries({ queryKey: ['thread', threadId] });
  };
}
