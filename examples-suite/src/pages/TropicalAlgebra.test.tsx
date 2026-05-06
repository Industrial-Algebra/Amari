import { describe, it, expect, vi } from 'vitest';
import { render, screen } from '../test/test-utils';
import { TropicalAlgebra } from './TropicalAlgebra';

vi.mock('../hooks/useAmariWasm', () => ({
  useAmariWasm: () => ({
    ready: false,
    error: null,
    amari: null,
  }),
}));

describe('TropicalAlgebra page', () => {
  it('documents the 0.21.0 WASM semiring and ordinal examples', () => {
    render(<TropicalAlgebra />);

    expect(screen.getByText('Semiring Folds via WASM')).toBeInTheDocument();
    expect(screen.getByText('Ordinal Weights Below ε₀ via WASM')).toBeInTheDocument();
    expect(screen.getByText(/TropicalBatch\.foldOplus/i)).toBeInTheDocument();
    expect(screen.getByText(/WasmOrdinalArena/i)).toBeInTheDocument();
  });

  it('does not describe tropical examples as simulated-only', () => {
    render(<TropicalAlgebra />);

    expect(screen.queryByText(/simulated tropical operations/i)).not.toBeInTheDocument();
  });
});
