import { useEffect, useRef } from 'react';
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
  hash,
  mime,
  onClose,
  onPrev,
  onNext,
  hasPrev,
  hasNext,
}: Props) {
  const dialogRef = useRef<HTMLDivElement>(null);
  // key-handling ------------------------------------------------------
  useEffect(() => {
    function handler(e: KeyboardEvent) {
      if (e.key === 'Escape') onClose();
      if (e.key === 'ArrowLeft' && onPrev) onPrev();
      if (e.key === 'ArrowRight' && onNext) onNext();
    }
    window.addEventListener('keydown', handler);
    dialogRef.current?.focus();
    return () => window.removeEventListener('keydown', handler);
  }, [onClose, onPrev, onNext]);
  // ------------------------------------------------------------------

  return (
    <div
      ref={dialogRef}
      role="dialog"
      aria-modal="true"
      aria-label="Attachment preview"
      tabIndex={-1}
      className="fixed inset-0 z-50 flex items-center justify-center bg-black bg-opacity-80"
      onClick={onClose}
    >
      <button
        type="button"
        className="btn btn-sm absolute right-4 top-4 z-10"
        onClick={(event) => {
          event.stopPropagation();
          onClose();
        }}
      >
        Close
      </button>
      {hasPrev && (
        <button
          type="button"
          className="btn absolute left-4 top-1/2 z-10 -translate-y-1/2"
          onClick={(e) => {
            e.stopPropagation();
            onPrev?.();
          }}
        >
          Previous
        </button>
      )}
      {hasNext && (
        <button
          type="button"
          className="btn absolute right-4 top-1/2 z-10 -translate-y-1/2"
          onClick={(e) => {
            e.stopPropagation();
            onNext?.();
          }}
        >
          Next
        </button>
      )}

      {mime?.startsWith('image/') && (
        <img
          className="max-w-full max-h-full"
          src={imageUrl(hash)}
          alt="Attachment preview"
          onClick={(e) => e.stopPropagation()}
        />
      )}
      {mime?.startsWith('video/') && (
        <video
          className="max-w-full max-h-full"
          controls
          autoPlay
          src={imageUrl(hash)}
          onClick={(e) => e.stopPropagation()}
        />
      )}
    </div>
  );
}
