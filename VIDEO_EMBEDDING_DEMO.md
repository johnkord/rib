# Video Embedding Feature Demo

## Overview

Automatic embedding for YouTube (incl. Music) and SoundCloud links. Embeds are user‑activated (click to load) and original URLs stay visible.

## Supported Platforms

### YouTube / YouTube Music
- Standard: `https://www.youtube.com/watch?v=VIDEO_ID`
- Short: `https://youtu.be/VIDEO_ID`
- Embed: `https://www.youtube.com/embed/VIDEO_ID`
- With params: `https://www.youtube.com/watch?v=VIDEO_ID&t=30s`
- Music: `https://music.youtube.com/watch?v=VIDEO_ID`

### SoundCloud
- Tracks / Playlists / Profiles: `https://soundcloud.com/{artist}/{track}`

(Former providers removed for leaner CSP.)

## How It Works
1. `linkifyText()` scans text
2. Detect provider (YouTube / SoundCloud)
3. Insert toggle component
4. On click: create iframe (lazy loaded)
5. CSP restricts frame domains

## Security
- Sandboxed iframes for SoundCloud
- Minimal feature policy allow list
- Tight CSP frame-src: YouTube + SoundCloud only

## Performance
- No provider network until toggle clicked
- Lazy loaded iframes

## Configuration
```
linkifyText(content)              // enable (default)
linkifyText(content, false)       // disable embedding
linkifyText(content, { enableVideoEmbeds: true, videoMaxWidthClass: 'max-w-md' })
```

## Testing
```
cd rib-react
npm test
```
Covers ID extraction, embed creation, and provider filtering.