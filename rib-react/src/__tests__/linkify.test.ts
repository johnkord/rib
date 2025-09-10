import { describe, it, expect } from 'vitest';
import { linkifyText, containsUrls, containsVideoUrls } from '../lib/linkify';
import { createElement } from 'react';

describe('linkifyText', () => {
  it('should return original text when no URLs are present', () => {
    const text = 'This is just plain text with no URLs';
    const result = linkifyText(text);
    
    expect(result).toHaveLength(1);
    expect(result[0]).toBe(text);
  });

  it('should convert HTTP URLs to clickable links when video embedding is disabled', () => {
    const text = 'Check out http://example.com for more info';
    const result = linkifyText(text, false); // Disable video embedding
    
    expect(result).toHaveLength(3);
    expect(result[0]).toBe('Check out ');
    expect(result[2]).toBe(' for more info');
    
    // Check that the middle element is a link
    const linkElement = result[1] as React.ReactElement;
    expect(linkElement.type).toBe('a');
    expect(linkElement.props.href).toBe('http://example.com');
    expect(linkElement.props.target).toBe('_blank');
    expect(linkElement.props.rel).toBe('noopener noreferrer');
    expect(linkElement.props.children).toBe('http://example.com');
  });

  it('should produce a toggle (not auto-embed) for YouTube videos when embedding enabled', () => {
    const text = 'Check out https://www.youtube.com/watch?v=dQw4w9WgXcQ';
    const result = linkifyText(text, true);
    expect(result).toHaveLength(2);
    expect(result[0]).toBe('Check out ');
    const toggle = result[1] as React.ReactElement;
    // Function component wrapper
    expect(typeof toggle.type).toBe('function');
  });

  it('should produce a toggle by default (no second arg) for video URLs', () => {
    const text = 'Video https://www.youtube.com/watch?v=dQw4w9WgXcQ here';
    const result = linkifyText(text);
    expect(result).toHaveLength(3);
    const toggle = result[1] as React.ReactElement;
    expect(typeof toggle.type).toBe('function');
  });

  it('should convert regular URLs to links when they are not video URLs', () => {
    const text = 'Visit https://example.com for info';
    const result = linkifyText(text, true);
    
    expect(result).toHaveLength(3);
    expect(result[0]).toBe('Visit ');
    expect(result[2]).toBe(' for info');
    
    const linkElement = result[1] as React.ReactElement;
    expect(linkElement.type).toBe('a');
    expect(linkElement.props.href).toBe('https://example.com');
  });

  it('should handle mixed content with video toggles and regular links', () => {
    const text = 'Watch https://www.youtube.com/watch?v=dQw4w9WgXcQ and visit https://example.com';
    const result = linkifyText(text, true);
    
    expect(result).toHaveLength(4);
    expect(result[0]).toBe('Watch ');
    expect(result[2]).toBe(' and visit ');
    
    // First should be toggle component (function type)
    const toggleElement = result[1] as React.ReactElement;
    expect(typeof toggleElement.type).toBe('function');
    
    // Second should be regular link
    const linkElement = result[3] as React.ReactElement;
    expect(linkElement.type).toBe('a');
    expect(linkElement.props.href).toBe('https://example.com');
  });

  it('should convert HTTPS URLs to clickable links', () => {
    const text = 'Secure site: https://secure-example.com/path?query=value';
    const result = linkifyText(text, false); // Disable embedding to test link behavior
    
    expect(result).toHaveLength(2);
    expect(result[0]).toBe('Secure site: ');
    
    const linkElement = result[1] as React.ReactElement;
    expect(linkElement.props.href).toBe('https://secure-example.com/path?query=value');
  });

  it('should handle multiple URLs in the same text', () => {
    const text = 'Visit http://example.com and https://another.com for info';
    const result = linkifyText(text, false); // Disable embedding to test link behavior
    
    expect(result).toHaveLength(5);
    expect(result[0]).toBe('Visit ');
    expect(result[2]).toBe(' and ');
    expect(result[4]).toBe(' for info');
    
    // Check both links
    const firstLink = result[1] as React.ReactElement;
    const secondLink = result[3] as React.ReactElement;
    
    expect(firstLink.props.href).toBe('http://example.com');
    expect(secondLink.props.href).toBe('https://another.com');
  });

  it('should handle URLs at the beginning and end of text', () => {
    const textStart = 'https://start.com is at the beginning';
    const resultStart = linkifyText(textStart, false);
    
    expect(resultStart).toHaveLength(2);
    expect((resultStart[0] as React.ReactElement).props.href).toBe('https://start.com');
    expect(resultStart[1]).toBe(' is at the beginning');

    const textEnd = 'The end has https://end.com';
    const resultEnd = linkifyText(textEnd, false);
    
    expect(resultEnd).toHaveLength(2);
    expect(resultEnd[0]).toBe('The end has ');
    expect((resultEnd[1] as React.ReactElement).props.href).toBe('https://end.com');
  });

  it('should handle empty or null text', () => {
    expect(linkifyText('')).toEqual([]);
    expect(linkifyText(null as any)).toEqual([]);
  });

  it('should preserve case in URLs', () => {
    const text = 'Visit https://Example.COM/Path';
    const result = linkifyText(text, false);
    
    const linkElement = result[1] as React.ReactElement;
    expect(linkElement.props.href).toBe('https://Example.COM/Path');
    expect(linkElement.props.children).toBe('https://Example.COM/Path');
  });

  it('should handle complex URLs with ports, fragments, etc', () => {
    const text = 'Complex: https://example.com:8080/path?q=1&r=2#fragment';
    const result = linkifyText(text, false);
    
    const linkElement = result[1] as React.ReactElement;
    expect(linkElement.props.href).toBe('https://example.com:8080/path?q=1&r=2#fragment');
  });
});

describe('containsUrls', () => {
  it('should return true for text with URLs', () => {
    expect(containsUrls('Check http://example.com')).toBe(true);
    expect(containsUrls('Secure https://example.com')).toBe(true);
    expect(containsUrls('Multiple http://one.com and https://two.com')).toBe(true);
  });

  it('should return false for text without URLs', () => {
    expect(containsUrls('Just plain text')).toBe(false);
    expect(containsUrls('No links here at all')).toBe(false);
    expect(containsUrls('')).toBe(false);
  });

  it('should handle malformed URLs correctly', () => {
    expect(containsUrls('Not a URL: www.example.com')).toBe(false);
    expect(containsUrls('Not a URL: ftp://example.com')).toBe(false);
    expect(containsUrls('Not a URL: example.com')).toBe(false);
  });
});

describe('containsVideoUrls', () => {
  it('returns true for supported video/audio URLs', () => {
    expect(containsVideoUrls('Check https://www.youtube.com/watch?v=dQw4w9WgXcQ')).toBe(true);
    expect(containsVideoUrls('https://soundcloud.com/forss/flickermood')).toBe(true);
  });
  it('returns false for unsupported providers', () => {
    expect(containsVideoUrls('Track https://open.spotify.com/track/11dFghVXANMlKmJXsNCbNl')).toBe(false);
  });
});