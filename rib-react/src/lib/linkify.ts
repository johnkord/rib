/**
 * Utility functions for converting URLs in text to clickable links
 */
import React from 'react';

// Regular expression to match URLs (http/https)
const URL_REGEX = /(https?:\/\/[^\s]+)/gi;

/**
 * Converts plain text containing URLs into an array of React nodes
 * that can be rendered with clickable links
 */
export function linkifyText(text: string): React.ReactNode[] {
  if (!text) return [];
  
  const parts: React.ReactNode[] = [];
  let lastIndex = 0;
  let match;
  
  // Reset regex lastIndex to ensure consistent behavior
  URL_REGEX.lastIndex = 0;
  
  while ((match = URL_REGEX.exec(text)) !== null) {
    const url = match[0];
    const matchIndex = match.index;
    
    // Add text before the URL
    if (matchIndex > lastIndex) {
      const beforeText = text.substring(lastIndex, matchIndex);
      parts.push(beforeText);
    }
    
    // Add the clickable URL using React.createElement
    parts.push(
      React.createElement(
        'a',
        {
          key: `link-${matchIndex}`,
          href: url,
          target: '_blank',
          rel: 'noopener noreferrer',
          className: 'text-blue-600 hover:text-blue-800 underline break-all'
        },
        url
      )
    );
    
    lastIndex = matchIndex + url.length;
  }
  
  // Add remaining text after the last URL
  if (lastIndex < text.length) {
    parts.push(text.substring(lastIndex));
  }
  
  // If no URLs were found, return the original text
  if (parts.length === 0) {
    parts.push(text);
  }
  
  return parts;
}

/**
 * Checks if the given text contains any URLs
 */
export function containsUrls(text: string): boolean {
  URL_REGEX.lastIndex = 0;
  return URL_REGEX.test(text);
}