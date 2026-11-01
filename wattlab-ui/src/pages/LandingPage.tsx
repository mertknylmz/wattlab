import { AppearanceBar } from '../components/AppearanceBar';
import { useAppearance } from '../i18n/appearance';

interface Props {
  onLogin(): void;
  onRegister(): void;
}

export function LandingPage({ onLogin, onRegister }: Props) {
  const { t } = useAppearance();
  return (
    <div className="auth-page">
      <section className="list-card landing-card" aria-labelledby="landing-title">
        <p className="brand landing-brand"><span className="brand-icon" aria-hidden="true">ϟ</span>Watt<span>Lab</span></p>
        <p className="eyebrow">{t('welcomeEyebrow')}</p>
        <h1 id="landing-title">{t('landingTitle')}</h1>
        <p className="landing-intro">{t('landingIntro')}</p>
        <div className="landing-actions">
          <button type="button" className="landing-primary" onClick={onLogin}>{t('landingLogin')}</button>
          <button type="button" className="secondary" onClick={onRegister}>{t('landingRegister')}</button>
        </div>
        <AppearanceBar />
      </section>
    </div>
  );
}
