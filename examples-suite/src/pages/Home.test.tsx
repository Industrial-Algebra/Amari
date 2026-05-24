import { describe, it, expect } from 'vitest';
import { render, screen } from '../test/test-utils';
import { Home } from './Home';

describe('Home Page', () => {
  it('renders the main title', () => {
    render(<Home />);

    expect(screen.getByRole('heading', { name: /Amari Mathematical Computing Library/i })).toBeInTheDocument();
  });

  it('renders the description', () => {
    render(<Home />);

    const descElements = screen.getAllByText(/Interactive/i);
    expect(descElements.length).toBeGreaterThan(0);
  });

  it('renders feature cards', () => {
    render(<Home />);

    // Check for key features mentioned on the home page - may have multiple matches
    const geometricElements = screen.getAllByText(/Geometric Algebra/i);
    expect(geometricElements.length).toBeGreaterThan(0);

    const tropicalElements = screen.getAllByText(/Tropical/i);
    expect(tropicalElements.length).toBeGreaterThan(0);

    const surcomplexElements = screen.getAllByText(/Rational Surcomplex/i);
    expect(surcomplexElements.length).toBeGreaterThan(0);
  });

  it('renders quick start section', () => {
    render(<Home />);

    const quickStartElements = screen.getAllByText(/Quick Start/i);
    expect(quickStartElements.length).toBeGreaterThan(0);
  });

  it('renders navigation links to examples', () => {
    render(<Home />);

    // The home page should have links to explore examples
    const exploreLinks = screen.getAllByRole('link');
    expect(exploreLinks.length).toBeGreaterThan(0);
  });
});
