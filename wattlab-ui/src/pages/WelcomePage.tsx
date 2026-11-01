import { useAppearance } from '../i18n/appearance';

export function WelcomePage() {
  const { t } = useAppearance();
  return (
    <section className="welcome" aria-labelledby="welcome-title">
      <div className="welcome-copy">
        <p className="eyebrow">{t('welcomeEyebrow')}</p>
        <h1 id="welcome-title">{t('welcomeTitle')}</h1>
        <p className="welcome-intro">{t('welcomeIntro')}</p>
        <a className="welcome-start" href="#/live">
          {t('getStarted')} <span aria-hidden="true">→</span>
        </a>
        <p className="welcome-hint">{t('welcomeHint')}</p>
      </div>
      <svg className="welcome-art" viewBox="0 0 360 320" fill="none" aria-hidden="true">
        <circle cx="180" cy="160" r="142" className="welcome-orbit" />
        <circle cx="180" cy="160" r="109" className="welcome-orbit" />
        <path d="M24 244h312M48 274h264" className="welcome-orbit" />
        <g stroke="currentColor" strokeWidth="5" strokeLinecap="round" strokeLinejoin="round">
          <circle cx="95" cy="205" r="49" />
          <circle cx="268" cy="205" r="49" />
          <path d="m95 205 52-91 50 91H95l117-65 56 65M197 205l-50-91h-22m78 0h29l-20 26" />
          <path d="m166 51-17 30h23l-15 27" />
        </g>
      </svg>
    </section>
  );
}
