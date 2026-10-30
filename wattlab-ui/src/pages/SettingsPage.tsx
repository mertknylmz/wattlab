import { useState, type FormEvent } from 'react';
import { UnauthorizedError, type ApiClient } from '../api/client';
import type { Settings } from '../api/types';
import { ZoneLegend } from '../components/ZoneLegend';
import { formatDate } from '../model/format';
import { isValidFtp } from '../model/zones';

interface Props {
  client: ApiClient;
  settings: Settings | null;
  onSaved(settings: Settings): void;
  onUnauthorized(): void;
}

export function SettingsPage({ client, settings, onSaved, onUnauthorized }: Props) {
  const [value, setValue] = useState('');
  const [message, setMessage] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);

  const save = async (ftp: number | null) => {
    setSaving(true);
    try {
      const saved = await client.saveFtp(ftp);
      onSaved(saved);
      setValue('');
      setMessage(ftp == null ? 'FTP varsayılan 200 W değerine döndü.' : `FTP ${saved.ftp_w} W olarak kaydedildi.`);
    } catch (error) {
      if (error instanceof UnauthorizedError) onUnauthorized();
      else setMessage('FTP kaydedilemedi · yerel API’ye ulaşılamıyor.');
    } finally {
      setSaving(false);
    }
  };

  const submit = (event: FormEvent) => {
    event.preventDefault();
    const ftp = Number(value);
    if (!isValidFtp(ftp)) {
      setMessage('FTP için 1–2000 arasında tam sayı watt gir.');
      return;
    }
    void save(ftp);
  };

  return (
    <>
      <section className="page-heading">
        <div>
          <p className="eyebrow">AYARLAR</p>
          <h1>Referansların.</h1>
          <p className="subtitle">FTP güç zonlarının referansıdır; trainer direncini değiştirmez.</p>
        </div>
      </section>
      <section className="list-card settings">
        <h2>FTP</h2>
        <p data-testid="ftp-current">
          {!settings
            ? 'Yükleniyor…'
            : settings.ftp_is_default
              ? `Varsayılan ${settings.ftp_w} W · kişisel ölçüm değildir.`
              : `${settings.ftp_w} W · ${formatDate(settings.updated_at)} tarihinde kaydedildi.`}
        </p>
        <form className="ftp-form" onSubmit={submit} noValidate>
          <label htmlFor="ftp">
            FTP <span>(W)</span>
          </label>
          <input
            id="ftp"
            type="number"
            min={1}
            max={2000}
            step={1}
            inputMode="numeric"
            value={value}
            onChange={(event) => setValue(event.target.value)}
          />
          <button type="submit" disabled={saving}>
            Kaydet
          </button>
          <button
            type="button"
            className="text-button"
            disabled={saving || !settings || settings.ftp_is_default}
            onClick={() => void save(null)}
          >
            Varsayılana dön
          </button>
        </form>
        {message && <p role="status">{message}</p>}
        <p className="hint">
          Yeni oturumlar başladıkları andaki FTP’yi saklar; FTP değişikliği eski oturumların zon dağılımını değiştirmez.
          FTP’si kaydedilmemiş eski oturumlar güncel FTP ile gösterilir.
        </p>
        <ZoneLegend />
      </section>
    </>
  );
}
