import { createContext, useContext, useEffect, useMemo, useState, type ReactNode } from 'react';
import { interpolate, MESSAGES, type Locale, type MessageKey, type Theme } from './messages';

const LOCALE_KEY = 'wattlab.locale';
const THEME_KEY = 'wattlab.theme';

function read<T extends string>(key: string, allowed: readonly T[], fallback: T): T {
  try {
    const value = localStorage.getItem(key);
    return value && (allowed as readonly string[]).includes(value) ? (value as T) : fallback;
  } catch {
    return fallback;
  }
}

function write(key: string, value: string) {
  try {
    localStorage.setItem(key, value);
  } catch {
    /* private mode */
  }
}

export type Translate = (key: MessageKey, values?: Record<string, string | number>) => string;

interface Appearance {
  locale: Locale;
  theme: Theme;
  t: Translate;
  setLocale(locale: Locale): void;
  setTheme(theme: Theme): void;
}

const Context = createContext<Appearance | null>(null);

function apply(locale: Locale, theme: Theme) {
  document.documentElement.lang = locale;
  document.documentElement.dataset.theme = theme;
}

export function AppearanceProvider({ children }: { children: ReactNode }) {
  const [locale, setLocaleState] = useState<Locale>(() => {
    const value = read(LOCALE_KEY, ['tr', 'en'], 'en');
    apply(value, read(THEME_KEY, ['dark', 'light'], 'dark'));
    return value;
  });
  const [theme, setThemeState] = useState<Theme>(() => read(THEME_KEY, ['dark', 'light'], 'dark'));

  useEffect(() => apply(locale, theme), [locale, theme]);

  const value = useMemo<Appearance>(() => {
    const t: Translate = (key, values) => interpolate(MESSAGES[locale][key], values);
    return {
      locale,
      theme,
      t,
      setLocale(next) {
        write(LOCALE_KEY, next);
        setLocaleState(next);
      },
      setTheme(next) {
        write(THEME_KEY, next);
        setThemeState(next);
      },
    };
  }, [locale, theme]);

  return <Context.Provider value={value}>{children}</Context.Provider>;
}

export function useAppearance(): Appearance {
  const value = useContext(Context);
  if (!value) throw new Error('AppearanceProvider is required');
  return value;
}

export function translate(locale: Locale, key: MessageKey, values?: Record<string, string | number>): string {
  return interpolate(MESSAGES[locale][key], values);
}
