import { useQuery, useQueryClient } from '@tanstack/react-query';
import { fetchJson, postJson, uploadImage } from '../lib/api';

export interface Reply {
	id: number;
	thread_id: number;
	content: string;
	image_hash?: string;    // ...unchanged...
	mime?: string;          // ...unchanged...
	created_at: string;     // NEW – ISO timestamp
}

export function useReplies(threadId: number | null) {
  return useQuery<Reply[]>({
    queryKey: ['replies', threadId],
    queryFn: () => fetchJson(`/threads/${threadId}/replies`),
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

    await postJson('/replies', { thread_id: threadId, content, image_hash, mime });
    await qc.invalidateQueries({ queryKey: ['replies', threadId] });
    await qc.invalidateQueries({ queryKey: ['thread', threadId] });
  };
}
