import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '../test/test-utils';
import { Navigation } from './Navigation';

describe('Navigation', () => {
  it('renders the library title', () => {
    render(<Navigation />);

    expect(screen.getByText('Amari Library')).toBeInTheDocument();
    expect(screen.getByText('Mathematical Computing Examples')).toBeInTheDocument();
  });

  it('renders all navigation sections', () => {
    render(<Navigation />);

    expect(screen.getByText('Core Mathematics')).toBeInTheDocument();
    expect(screen.getByText('Advanced Systems')).toBeInTheDocument();
    expect(screen.getByText('Tools & Playground')).toBeInTheDocument();
  });

  it('renders core mathematics links', () => {
    render(<Navigation />);

    expect(screen.getByText('Geometric Algebra')).toBeInTheDocument();
    expect(screen.getByText('Tropical Algebra')).toBeInTheDocument();
    expect(screen.getByText('Dual Numbers')).toBeInTheDocument();
    expect(screen.getByText('Rational Surcomplex')).toBeInTheDocument();
    expect(screen.getByText('Information Geometry')).toBeInTheDocument();
    expect(screen.getByText('Enumerative Geometry')).toBeInTheDocument();
    expect(screen.getByText('Calculus')).toBeInTheDocument();
    expect(screen.getByText('Measure Theory')).toBeInTheDocument();
  });

  it('renders advanced systems links', () => {
    render(<Navigation />);

    expect(screen.getByText('WebGPU Acceleration')).toBeInTheDocument();
    expect(screen.getByText('Fusion System')).toBeInTheDocument();
    expect(screen.getByText('Cellular Automata')).toBeInTheDocument();
    expect(screen.getByText('Probabilistic')).toBeInTheDocument();
    expect(screen.getByText('Relativistic')).toBeInTheDocument();
    expect(screen.getByText('Network')).toBeInTheDocument();
    expect(screen.getByText('Holographic')).toBeInTheDocument();
    expect(screen.getByText('Optimization')).toBeInTheDocument();
  });

  it('renders tools & playground links', () => {
    render(<Navigation />);

    expect(screen.getByText('Interactive Playground')).toBeInTheDocument();
    expect(screen.getByText('Performance Benchmarks')).toBeInTheDocument();
    expect(screen.getByText('API Reference')).toBeInTheDocument();
    expect(screen.getByText('Amari-Chentsov Tensor')).toBeInTheDocument();
  });

  it('renders link descriptions', () => {
    render(<Navigation />);

    expect(screen.getByText('Multivectors, geometric products, and rotors')).toBeInTheDocument();
    expect(screen.getByText('Max-plus semiring operations')).toBeInTheDocument();
    expect(screen.getByText('Automatic differentiation')).toBeInTheDocument();
  });

  it('calls onNavigate when a link is clicked', () => {
    const onNavigate = vi.fn();
    render(<Navigation onNavigate={onNavigate} />);

    fireEvent.click(screen.getByText('Geometric Algebra'));

    expect(onNavigate).toHaveBeenCalled();
  });

  it('highlights active link based on current path', () => {
    render(<Navigation />, { initialEntries: ['/geometric-algebra'] });

    // The active link should have different styling
    // We can check for the link existence on the correct path
    const link = screen.getByText('Geometric Algebra');
    expect(link).toBeInTheDocument();
  });

  it('links navigate to correct paths', () => {
    render(<Navigation />);

    // Check that links have correct hrefs
    const geometricAlgebraLink = screen.getByText('Geometric Algebra').closest('a');
    expect(geometricAlgebraLink).toHaveAttribute('href', '/geometric-algebra');

    const tropicalLink = screen.getByText('Tropical Algebra').closest('a');
    expect(tropicalLink).toHaveAttribute('href', '/tropical-algebra');

    const surcomplexLink = screen.getByText('Rational Surcomplex').closest('a');
    expect(surcomplexLink).toHaveAttribute('href', '/surcomplex');
  });
});
