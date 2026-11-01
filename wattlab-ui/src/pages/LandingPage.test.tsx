import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { App } from '../App';
import { ApiClient } from '../api/client';

const KEY = 'wattlab.rememberedUsername';
const input = (label: string) => screen.getByLabelText(label) as HTMLInputElement;

function mount() {
  const fetcher = vi.fn((url: string) => {
    const value = url === '/api/v1/desktop/capabilities' ? { remember_available: true } : {};
    return Promise.resolve(new Response(JSON.stringify(value), { status: url === '/api/v1/auth/me' ? 401 : 200 }));
  });
  render(<App client={new ApiClient(fetcher as unknown as typeof fetch)} />);
  return fetcher;
}

afterEach(() => {
  cleanup();
  localStorage.clear();
  document.documentElement.removeAttribute('data-theme');
  window.history.replaceState(null, '', '#/welcome');
});

describe('pre-login landing', () => {
  it('opens remembered login without exposing a username field and clears passwords when returning', async () => {
    localStorage.setItem(KEY, 'rider');
    localStorage.setItem('wattlab.trainerAddress', 'AA:BB:CC:DD:EE:FF');
    const fetcher = mount();
    await screen.findByRole('heading', { name: 'Your next ride starts here.' });
    expect(document.querySelector('form')).toBeNull();
    expect(screen.queryByLabelText('Password')).toBeNull();
    expect(screen.queryByText('Welcome back, rider')).toBeNull();
    fireEvent.click(screen.getByRole('button', { name: 'Log in' }));
    expect(screen.getByRole('heading', { name: 'Welcome back, rider' })).toBeTruthy();
    expect(screen.queryByLabelText('Username')).toBeNull();
    expect(input('Password').value).toBe('');
    fireEvent.change(input('Password'), { target: { value: 'unsent password' } });
    fireEvent.click(screen.getByRole('button', { name: 'Back to start' }));
    expect(screen.getByRole('heading', { name: 'Your next ride starts here.' })).toBeTruthy();
    fireEvent.click(screen.getByRole('button', { name: 'Log in' }));
    expect(input('Password').value).toBe('');
    expect(localStorage.getItem(KEY)).toBe('rider');
    expect(fetcher.mock.calls.map(([url]) => url)).toEqual([
      '/api/v1/desktop/capabilities', '/api/v1/auth/me',
    ]);
  });

  it('opens blank registration directly even with a remembered username, then returns to remembered login', async () => {
    localStorage.setItem(KEY, 'rider');
    mount();
    fireEvent.click(await screen.findByRole('button', { name: 'Register' }));
    expect(screen.getByRole('heading', { name: 'Create account' })).toBeTruthy();
    for (const field of ['Username', 'Password', 'Repeat password']) expect(input(field).value).toBe('');
    fireEvent.click(screen.getByRole('button', { name: 'Sign in' }));
    expect(screen.getByRole('heading', { name: 'Welcome back, rider' })).toBeTruthy();
    expect(screen.queryByLabelText('Username')).toBeNull();
  });

  it('localizes the landing and forms and preserves the selected theme across navigation', async () => {
    mount();
    await screen.findByRole('button', { name: 'Log in' });
    fireEvent.click(screen.getByRole('button', { name: 'TR' }));
    expect(screen.getByRole('heading', { name: 'Bir sonraki sürüşün burada başlar.' })).toBeTruthy();
    fireEvent.click(screen.getByRole('button', { name: 'Açık tema' }));
    expect(document.documentElement.dataset.theme).toBe('light');
    fireEvent.click(screen.getByRole('button', { name: 'Giriş yap' }));
    expect(input('Kullanıcı adı').value).toBe('');
    expect(input('Şifre').value).toBe('');
    expect(document.documentElement.dataset.theme).toBe('light');
    fireEvent.click(screen.getByRole('button', { name: 'Başlangıca dön' }));
    fireEvent.click(screen.getByRole('button', { name: 'Kayıt ol' }));
    expect(input('Şifre tekrar').value).toBe('');
    expect(document.documentElement.dataset.theme).toBe('light');
  });
});
