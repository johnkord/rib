import { describe, it, expect } from 'vitest';
import { About } from '../pages/About';
import { render, screen } from '@testing-library/react';

describe('About (About) page', () => {
  it('renders about and operator details', () => {
    render(<About />);
    expect(screen.getByText(/About/)).toBeTruthy();
    expect(screen.getAllByText(/John Kordich/).length).toBeGreaterThan(0);
  expect(screen.getByText(/jkordich/)).toBeTruthy();
  expect(screen.getAllByText(/curlyquote/).length).toBeGreaterThan(0);
    expect(screen.getByText(/bc1qlawxetusaugute86w3yc8m72xggak5lkjgqd2p/)).toBeTruthy();
    expect(screen.getByText(/github.com\/johnkord\/rib/)).toBeTruthy();
  });
});
