import { describe, it, expect, vi } from 'vitest';
import { render, screen } from '../test/test-utils';
import { DualNumbers } from './DualNumbers';

vi.mock('../hooks/useAmariWasm', () => ({
  useAmariWasm: () => ({
    ready: false,
    error: null,
    amari: null,
  }),
}));

describe('DualNumbers page', () => {
  it('documents the 0.21.0 WASM branch-policy and static multi-dual examples', () => {
    render(<DualNumbers />);

    expect(screen.getByText('Branch Policies via WASM')).toBeInTheDocument();
    expect(screen.getByText('Multi-Dual Seeding via WASM')).toBeInTheDocument();
    expect(screen.getByText('Static Multi-Dual Hot Loop via WASM')).toBeInTheDocument();
    expect(screen.getAllByText(/WasmBranchPolicy\.Average/i).length).toBeGreaterThan(0);
    expect(screen.getAllByText(/WasmStaticMultiDual2/i).length).toBeGreaterThan(0);
  });
});
