// A local form preference only. Session tokens remain in Electron; passwords
// must never be passed to or persisted by this module.
const USERNAME_KEY = 'wattlab.rememberedUsername';
const USERNAME = /^[a-z0-9_.-]{3,32}$/;

export function readRememberedUsername(): string | null {
  try {
    const value = localStorage.getItem(USERNAME_KEY);
    return value && USERNAME.test(value) ? value : null;
  } catch {
    return null;
  }
}

export function writeRememberedUsername(username: string | null): void {
  try {
    if (username && USERNAME.test(username)) localStorage.setItem(USERNAME_KEY, username);
    else localStorage.removeItem(USERNAME_KEY);
  } catch {
    // Unavailable preference storage must not prevent sign-in or logout.
  }
}
