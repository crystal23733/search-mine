import { render, screen } from '@testing-library/preact';
import { expect, test } from 'vitest';
import { App } from './App';

test('boots the public page with one accessible product heading', () => {
  render(<App />);
  expect(screen.getByRole('heading', { level: 1, name: 'Liar Sweeper' }).isConnected).toBe(true);
  expect(screen.queryByLabelText(/비밀번호|password/i)).toBeNull();
});
