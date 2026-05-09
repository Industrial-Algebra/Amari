import { describe, it, expect } from 'vitest';
import { render, screen } from '../test/test-utils';
import { CgtSurreal } from './CgtSurreal';

describe('CgtSurreal page', () => {
  it('documents v0.22.0 CGT and surreal WASM examples', () => {
    render(<CgtSurreal />);

    expect(screen.getByRole('heading', { name: /CGT & Short Surreals/i })).toBeInTheDocument();
    expect(screen.getAllByText(/WasmCgtArena/i).length).toBeGreaterThan(0);
    expect(screen.getAllByText(/WasmShortSurreal/i).length).toBeGreaterThan(0);
    expect(screen.getAllByText(/short normal-play games/i).length).toBeGreaterThan(0);
  });
});
