import { useAppearance } from '../i18n/appearance';

export function AppearanceBar() {
  const { locale, theme, t, setLocale, setTheme } = useAppearance();
  return (
    <div className="appearance-bar">
      <div className="locale-toggle" role="group" aria-label={t('language')}>
        <button type="button" aria-pressed={locale === 'tr'} onClick={() => setLocale('tr')}>
          TR
        </button>
        <button type="button" aria-pressed={locale === 'en'} onClick={() => setLocale('en')}>
          EN
        </button>
      </div>
      <button
        type="button"
        className="theme-toggle"
        aria-pressed={theme === 'light'}
        aria-label={theme === 'light' ? t('themeDark') : t('themeLight')}
        onClick={() => setTheme(theme === 'dark' ? 'light' : 'dark')}
      >
        <svg className="rail-icon" viewBox="0 0 24 24" aria-hidden="true">
          <path
            fill="currentColor"
            d={
              theme === 'dark'
                ? 'M6.8 12a5.2 5.2 0 1 0 10.4 0 5.2 5.2 0 0 0-10.4 0zM11 1h2v3h-2zm0 19h2v3h-2zM1 11h3v2H1zm19 0h3v2h-3zM4.2 3.8l1.4-1.4 2.1 2.1-1.4 1.4zm11.7 11.7 2.1 2.1-1.4 1.4-2.1-2.1zm2.1-11.7-1.4-1.4-2.1 2.1 1.4 1.4zM7.7 16.3 5.6 18.4 4.2 17l2.1-2.1z'
                : 'M12.7 2.3a9 9 0 1 0 9 11.4 7 7 0 0 1-9-11.4z'
            }
          />
        </svg>
      </button>
    </div>
  );
}
