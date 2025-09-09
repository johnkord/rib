import { useEffect } from 'react';
import { imageUrl } from '../lib/api';

interface Props {
  hash: string;
  mime: string | null | undefined;
  onClose: () => void;
  onPrev?: () => void;
  onNext?: () => void;
  hasPrev?: boolean; // for UI hint
  hasNext?: boolean;
}

export default function MediaModal({
  hash, mime, onClose, onPrev, onNext, hasPrev, hasNext,
}: Props) {

  // key-handling ------------------------------------------------------
  useEffect(() => {
    function handler(e: KeyboardEvent) {
      if (e.key === 'Escape') onClose();
      if (e.key === 'ArrowLeft' && onPrev) onPrev();
      if (e.key === 'ArrowRight' && onNext) onNext();
    }
    window.addEventListener('keydown', handler);
    return () => window.removeEventListener('keydown', handler);
  }, [onClose, onPrev, onNext]);
  // ------------------------------------------------------------------

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black bg-opacity-80"
      onClick={onClose}
    >
      {/* prev / next click-zones -------------------------------------- */}
      {hasPrev && <div className="absolute left-0 top-0 h-full w-1/4 cursor-pointer" onClick={(e)=>{e.stopPropagation(); onPrev?.();}} />}
      {hasNext && <div className="absolute right-0 top-0 h-full w-1/4 cursor-pointer" onClick={(e)=>{e.stopPropagation(); onNext?.();}} />}
      {/* -------------------------------------------------------------- */}

      {mime?.startsWith('image/') && (
        <img
          className="max-w-full max-h-full"
          src={imageUrl(hash)}
          onClick={e => e.stopPropagation()}
        />
      )}
      {mime?.startsWith('video/') && (
        <video
          className="max-w-full max-h-full"
          controls
          autoPlay
          src={imageUrl(hash)}
          onClick={e => e.stopPropagation()}
        />
      )}
    </div>
  );
}
