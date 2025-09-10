/**
 * Video embedding utilities (YouTube + SoundCloud only)
 */
import React from 'react';

export interface VideoEmbed {
  type: 'youtube' | 'soundcloud';
  videoId: string;
  embedUrl: string;
  thumbnailUrl?: string;
  originalUrl?: string;
}

// Extract YouTube ID -------------------------------------------------
export function extractYouTubeId(url: string): string | null {
  const patterns = [
    /(?:youtube\.com\/watch\?v=|music\.youtube\.com\/watch\?v=|youtu\.be\/|youtube\.com\/embed\/)([a-zA-Z0-9_-]{11})/,
    /(?:youtube|music\.youtube)\.com\/watch\?.*v=([a-zA-Z0-9_-]{11})/
  ];
  for (const pattern of patterns) {
    const m = url.match(pattern); if (m) return m[1];
  }
  return null;
}

// SoundCloud ---------------------------------------------------------
export function isSoundCloud(url: string): boolean { return /https?:\/\/(?:on\.)?soundcloud\.com\//.test(url); }

// Detect embeddable --------------------------------------------------
export function detectVideoEmbed(url: string): VideoEmbed | null {
  const yt = extractYouTubeId(url);
  if (yt) return { type: 'youtube', videoId: yt, embedUrl: `https://www.youtube.com/embed/${yt}`, thumbnailUrl: `https://img.youtube.com/vi/${yt}/maxresdefault.jpg` };
  if (isSoundCloud(url)) {
    const idPart = url.split('/').slice(-1)[0].slice(0, 32);
    const encoded = encodeURIComponent(url);
    return { type: 'soundcloud', videoId: idPart || 'sc', embedUrl: `https://w.soundcloud.com/player/?url=${encoded}&auto_play=false`, originalUrl: url };
  }
  return null;
}

export interface CreateVideoOptions { widthClass?: string }

// Create embed -------------------------------------------------------
export function createVideoEmbed(embed: VideoEmbed, originalUrl: string, opts?: CreateVideoOptions): React.ReactElement {
  let width = '560';
  let height = embed.type === 'soundcloud' ? '166' : '315';
  const commonProps = {
    width,
    height,
    frameBorder: '0',
    allowFullScreen: true,
    loading: 'lazy' as const,
    className: `${opts?.widthClass ?? ''} rounded-lg shadow-md mb-4`,
    title: `${embed.type} player`
  };
  return React.createElement('div', { key: `video-${embed.videoId}`, className: 'video-embed-container mb-4' }, [
    React.createElement('iframe', {
      ...commonProps,
      key: `iframe-${embed.videoId}`,
      src: embed.embedUrl,
      allow: embed.type === 'youtube'
        ? 'autoplay; clipboard-write; encrypted-media; picture-in-picture'
        : 'autoplay',
      ...(embed.type === 'youtube' ? { referrerPolicy: 'strict-origin-when-cross-origin' } : { sandbox: 'allow-scripts allow-same-origin allow-presentation allow-popups' })
    }),
    React.createElement('div', { key: `link-${embed.videoId}`, className: 'text-xs text-gray-500 mt-1' }, [
      React.createElement('a', { key: `anchor-${embed.videoId}`, href: originalUrl, target: '_blank', rel: 'noopener noreferrer', className: 'hover:text-gray-700 underline' }, originalUrl)
    ])
  ]);
}

export function shouldEmbed(url: string): boolean { return detectVideoEmbed(url) !== null; }