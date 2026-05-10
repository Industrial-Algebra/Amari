import { describe, it, expect } from 'vitest';
import { render, screen } from '../test/test-utils';
import { Surcomplex } from './Surcomplex';

describe('Surcomplex page', () => {
  it('documents rational surreal, epsilon, and surcomplex examples', async () => {
    render(<Surcomplex />);

    expect(screen.getByRole('heading', { name: /Rational Surreal & Surcomplex/i })).toBeInTheDocument();
    expect(await screen.findByText(/Rational Surreals/i)).toBeInTheDocument();
    expect(screen.getAllByText(/Surcomplex Division/i).length).toBeGreaterThan(0);
    expect(screen.getAllByText(/Experimental Epsilon/i).length).toBeGreaterThan(0);
    expect(screen.getAllByText(/4\/5 - 2\/5i/i).length).toBeGreaterThan(0);
  });
});
