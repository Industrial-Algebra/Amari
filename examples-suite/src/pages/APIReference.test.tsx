import { describe, it, expect } from 'vitest';
import { fireEvent, render, screen } from '../test/test-utils';
import { APIReference } from './APIReference';

describe('APIReference page', () => {
  it('lists the 0.21.0 tropical WASM ordinal and semiring APIs', () => {
    render(<APIReference />);

    fireEvent.click(screen.getByText('Tropical Algebra'));

    expect(screen.getByText('WasmOrdinalArena')).toBeInTheDocument();
    expect(screen.getByText('WasmOrdinalWeight')).toBeInTheDocument();
    expect(screen.getByText('foldOplus')).toBeInTheDocument();
    expect(screen.getByText('foldOtimes')).toBeInTheDocument();
  });

  it('lists the 0.21.0 dual WASM branch policy and static multi-dual APIs', () => {
    render(<APIReference />);

    fireEvent.click(screen.getByText('Dual Numbers & Autodiff'));

    expect(screen.getByText('WasmBranchPolicy')).toBeInTheDocument();
    expect(screen.getByText('WasmStaticMultiDual2')).toBeInTheDocument();
    expect(screen.getAllByText('variables').length).toBeGreaterThan(0);
    expect(screen.getAllByText('maxByPolicy').length).toBeGreaterThan(0);
  });
});
