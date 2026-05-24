import { describe, it, expect } from 'vitest';
import { fireEvent, render, screen } from '../test/test-utils';
import { APIReference } from './APIReference';

describe('APIReference page', () => {
  it('lists the 0.22.0 CGT and surreal WASM APIs', () => {
    render(<APIReference />);

    fireEvent.click(screen.getByText('CGT & Short Surreals'));

    expect(screen.getByText('WasmCgtArena')).toBeInTheDocument();
    expect(screen.getByText('WasmGameInspection')).toBeInTheDocument();
    expect(screen.getByText('WasmDyadic')).toBeInTheDocument();
    expect(screen.getByText('WasmShortSurreal')).toBeInTheDocument();
  });

  it('lists the 0.21.0 tropical WASM ordinal and semiring APIs', () => {
    render(<APIReference />);

    fireEvent.click(screen.getByText('Tropical Algebra'));

    expect(screen.getByText('WasmOrdinalArena')).toBeInTheDocument();
    expect(screen.getByText('WasmOrdinalWeight')).toBeInTheDocument();
    expect(screen.getByText('foldOplus')).toBeInTheDocument();
    expect(screen.getByText('foldOtimes')).toBeInTheDocument();
  });

  it('lists the 0.23.0 rational surreal and surcomplex WASM APIs', () => {
    render(<APIReference />);

    fireEvent.click(screen.getByText('Rational Surreal & Surcomplex'));

    expect(screen.getByText('WasmRationalSurreal')).toBeInTheDocument();
    expect(screen.getByText('WasmRationalSurcomplex')).toBeInTheDocument();
    expect(screen.getByText('WasmExperimentalEpsilonRational')).toBeInTheDocument();
    expect(screen.getByText('fromRatio')).toBeInTheDocument();
    expect(screen.getByText('fromParts')).toBeInTheDocument();
    expect(screen.getByText('fromScalar')).toBeInTheDocument();
    expect(screen.getByText('checkedReciprocal')).toBeInTheDocument();
    expect(screen.getAllByText('checkedDiv').length).toBe(2);
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
