/**
 * Utility functions for converting URLs in text to clickable links and video embeds
 */
import React, { useState } from 'react';
import { detectVideoEmbed, createVideoEmbed, VideoEmbed } from './videoEmbed';

interface LinkifyOptions {
  enableVideoEmbeds?: boolean;
  videoMaxWidthClass?: string; // e.g. 'max-w-md', 'max-w-lg'
}

// Regular expression to match URLs (http/https)
const URL_REGEX = /(https?:\/\/[^\s]+)/gi;

/**
 * Toggle component (defined once per linkifyText call) that reveals the iframe on demand.
 */
const makeVideoToggle = (videoMaxWidthClass?: string) => {
  const VideoToggle: React.FC<{ embed: VideoEmbed; originalUrl: string }> = ({ embed, originalUrl }) => {
    const [show, setShow] = useState(false);
    if (show) {
      return createVideoEmbed(embed, originalUrl, { widthClass: videoMaxWidthClass });
    }
    return React.createElement(
      'span',
      {
        className: 'video-embed-toggle inline-flex items-center gap-1',
        'data-type': 'video-toggle',
        'data-video-type': embed.type,
        'data-original-url': originalUrl,
      } as any,
      [
        React.createElement('a', {
          key: 'lnk',
          href: originalUrl,
          target: '_blank',
          rel: 'noopener noreferrer',
          className: 'text-blue-600 hover:text-blue-800 underline break-all'
        }, originalUrl),
        React.createElement('button', {
          key: 'btn',
          type: 'button',
            className: 'text-xs text-blue-600 hover:text-blue-800 underline',
          onClick: () => setShow(true)
        }, '[Embed]')
      ]
    );
  };
  return VideoToggle;
};

/**
 * Converts plain text containing URLs into an array of React nodes
 * that can be rendered with clickable links or embedded videos
 */
export function linkifyText(text: string, options?: boolean | LinkifyOptions): React.ReactNode[] {
  if (!text) return [];
  let enableVideoEmbeds = true;
  let videoMaxWidthClass: string | undefined;
  if (typeof options === 'boolean') {
    enableVideoEmbeds = options;
  } else if (typeof options === 'object') {
    enableVideoEmbeds = options.enableVideoEmbeds ?? true;
    videoMaxWidthClass = options.videoMaxWidthClass;
  }

  const VideoToggle = enableVideoEmbeds ? makeVideoToggle(videoMaxWidthClass) : null;

  const parts: React.ReactNode[] = [];
  let lastIndex = 0;
  let match: RegExpExecArray | null;

  URL_REGEX.lastIndex = 0; // safety reset

  while ((match = URL_REGEX.exec(text)) !== null) {
    const url = match[0];
    const matchIndex = match.index;

    if (matchIndex > lastIndex) {
      parts.push(text.substring(lastIndex, matchIndex));
    }

    const videoEmbed = enableVideoEmbeds ? detectVideoEmbed(url) : null;

    if (videoEmbed && VideoToggle) {
      parts.push(React.createElement(VideoToggle, { key: `video-${matchIndex}`, embed: videoEmbed, originalUrl: url }));
    } else {
      parts.push(React.createElement(
        'a',
        {
          key: `link-${matchIndex}`,
          href: url,
          target: '_blank',
          rel: 'noopener noreferrer',
          className: 'text-blue-600 hover:text-blue-800 underline break-all'
        },
        url
      ));
    }

    lastIndex = matchIndex + url.length;
  }

  if (lastIndex < text.length) {
    parts.push(text.substring(lastIndex));
  }

  if (parts.length === 0) parts.push(text);
  return parts;
}

/**
 * Checks if the given text contains any URLs
 */
export function containsUrls(text: string): boolean {
  if (!text) return false;
  URL_REGEX.lastIndex = 0;
  return URL_REGEX.test(text);
}

/**
 * Checks if the given text contains any video URLs that can be embedded
 */
export function containsVideoUrls(text: string): boolean {
  if (!text) return false;
  URL_REGEX.lastIndex = 0;
  let match: RegExpExecArray | null;
  while ((match = URL_REGEX.exec(text)) !== null) {
    if (detectVideoEmbed(match[0])) return true;
  }
  return false;
}