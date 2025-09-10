import { describe, it, expect } from 'vitest';
import { extractYouTubeId, detectVideoEmbed, shouldEmbed } from '../lib/videoEmbed';
import { createVideoEmbed } from '../lib/videoEmbed';

describe('extractYouTubeId', () => {
  it('should extract ID from standard YouTube URLs', () => {
    expect(extractYouTubeId('https://www.youtube.com/watch?v=dQw4w9WgXcQ')).toBe('dQw4w9WgXcQ');
    expect(extractYouTubeId('http://youtube.com/watch?v=dQw4w9WgXcQ')).toBe('dQw4w9WgXcQ');
  });

  it('supports custom width class via options', () => {
    const embed = detectVideoEmbed('https://www.youtube.com/watch?v=dQw4w9WgXcQ');
    expect(embed).not.toBeNull();
    if (embed) {
      const el = createVideoEmbed(embed, 'https://www.youtube.com/watch?v=dQw4w9WgXcQ', { widthClass: 'max-w-sm' });
      const iframe = (el.props.children as any[]).find((c: any) => c.type === 'iframe');
      expect(iframe.props.className).toContain('max-w-sm');
    }
  });

  it('should extract ID from shortened YouTube URLs', () => {
    expect(extractYouTubeId('https://youtu.be/dQw4w9WgXcQ')).toBe('dQw4w9WgXcQ');
    expect(extractYouTubeId('http://youtu.be/dQw4w9WgXcQ')).toBe('dQw4w9WgXcQ');
  });

  it('should extract ID from embed URLs', () => {
    expect(extractYouTubeId('https://www.youtube.com/embed/dQw4w9WgXcQ')).toBe('dQw4w9WgXcQ');
  });

  it('should extract ID from URLs with additional parameters', () => {
    expect(extractYouTubeId('https://www.youtube.com/watch?v=dQw4w9WgXcQ&t=30s')).toBe('dQw4w9WgXcQ');
    expect(extractYouTubeId('https://www.youtube.com/watch?list=PLrAXtmRdnEQy8M2gEBz2NQpSYHKGz9E3M&v=dQw4w9WgXcQ')).toBe('dQw4w9WgXcQ');
  });

  it('should return null for non-YouTube URLs', () => {
    expect(extractYouTubeId('https://example.com')).toBeNull();
    expect(extractYouTubeId('https://vimeo.com/123456')).toBeNull();
  });

  it('should return null for invalid YouTube URLs', () => {
    expect(extractYouTubeId('https://youtube.com/watch?v=')).toBeNull();
    expect(extractYouTubeId('https://youtube.com/watch')).toBeNull();
  });
});

// Removed extractVimeoId and extractTwitchId tests (providers no longer supported)

describe('detectVideoEmbed', () => {
  it('should detect YouTube embeds', () => {
    const embed = detectVideoEmbed('https://www.youtube.com/watch?v=dQw4w9WgXcQ');
    expect(embed).toEqual({
      type: 'youtube',
      videoId: 'dQw4w9WgXcQ',
      embedUrl: 'https://www.youtube.com/embed/dQw4w9WgXcQ',
      thumbnailUrl: 'https://img.youtube.com/vi/dQw4w9WgXcQ/maxresdefault.jpg'
    });
  });

  it('should create iframe without sandbox for YouTube to avoid error 153', () => {
    const embed = detectVideoEmbed('https://www.youtube.com/watch?v=dQw4w9WgXcQ');
    expect(embed).not.toBeNull();
    if (embed) {
      const el = createVideoEmbed(embed, 'https://www.youtube.com/watch?v=dQw4w9WgXcQ');
      // container div children
      const iframe = (el.props.children as any[]).find((c: any) => c.type === 'iframe');
      expect(iframe.props.sandbox).toBeUndefined();
  // Trimmed allow list: ensure expected subset still present
  expect(iframe.props.allow).toContain('autoplay');
  expect(iframe.props.allow).not.toContain('web-share');
      expect(iframe.props.referrerPolicy).toBe('strict-origin-when-cross-origin');
    }
  });

  // Removed Vimeo & Twitch detection tests

  it('should return null for non-video URLs', () => {
    expect(detectVideoEmbed('https://example.com')).toBeNull();
    expect(detectVideoEmbed('https://github.com/user/repo')).toBeNull();
  });

  it('should detect SoundCloud embeds', () => {
    const url = 'https://soundcloud.com/forss/flickermood';
    const embed = detectVideoEmbed(url)!;
    expect(embed.type).toBe('soundcloud');
    expect(embed.embedUrl.startsWith('https://w.soundcloud.com/player/?url=')).toBe(true);
  });

  // Removed Bandcamp detection test

  // Removed X (Twitter) detection test
});

describe('shouldEmbed', () => {
  it('should return true for video URLs', () => {
    expect(shouldEmbed('https://www.youtube.com/watch?v=dQw4w9WgXcQ')).toBe(true);
    expect(shouldEmbed('https://soundcloud.com/forss/flickermood')).toBe(true);
  });

  it('should return false for non-video URLs', () => {
    expect(shouldEmbed('https://example.com')).toBe(false);
    expect(shouldEmbed('https://github.com/user/repo')).toBe(false);
  });
});