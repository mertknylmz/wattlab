import { useState, type FormEvent } from 'react';
import type { ApiClient } from '../api/client';
import type { AgentState, CommandKind } from '../api/types';
import { useCommand, type CommandState } from '../hooks/useCommand';

const ACTIONS: Record<CommandKind, string> = {
  connect: 'Bağlantı',
  disconnect: 'Bağlantıyı kesme',
  start_session: 'Oturum başlatma',
  finish_session: 'Oturum bitirme',
  set_target_power: 'Hedef güç',
  release_control: 'Direnci bırakma',
};

const REASONS: Record<string, string> = {
  session_already_active: 'zaten aktif bir oturum var',
  no_active_session: 'aktif oturum yok',
  session_not_created: 'oturum API’de oluşturulamadı',
  trainer_not_configured: 'trainer adresi yapılandırılmamış',
  erg_unavailable: 'trainer bağlı değil veya hedef gücü desteklemiyor',
  target_out_of_range: 'hedef güç trainer aralığının dışında',
  control_not_permitted: 'trainer kontrol iznini vermedi',
  op_code_not_supported: 'trainer bu komutu desteklemiyor',
  invalid_parameter: 'trainer değeri geçersiz buldu',
  operation_failed: 'trainer işlemi tamamlayamadı',
  control_failed: 'trainer yanıt vermedi',
  disconnect_timeout: 'bağlantı süresinde kapanmadı',
};

const STATES: Record<string, string> = {
  idle: 'Bağlı değil',
  connecting: 'Bağlanıyor',
  subscribing: 'Veri kanalları hazırlanıyor',
  streaming: 'Bağlı · veri akıyor',
  retry_wait: 'Yeniden bağlantı bekleniyor',
  stopped: 'Durdu',
  failed: 'Bağlantı başarısız · yeniden bağlanmayı dene',
};

export function describeCommand(state: CommandState | null): string {
  if (!state) return 'Komutlar trainer ajanına iletilir; sonucu burada görünür.';
  const action = ACTIONS[state.kind];
  switch (state.status) {
    case 'pending':
    case 'delivered':
      return `${action} isteği gönderildi · trainer ajanının yanıtı bekleniyor.`;
    case 'succeeded':
      return `${action} tamamlandı.`;
    case 'rejected':
      return `${action} reddedildi: ${REASONS[state.message ?? ''] ?? 'ajan nedeni bildirmedi'}.`;
    case 'expired':
      return `${action} zaman aşımına uğradı · trainer ajanı çalışmıyor olabilir.`;
    default:
      return `${action} isteği API’ye ulaşmadı.`;
  }
}

interface Props {
  client: ApiClient;
  agent: AgentState | null;
  sessionActive: boolean;
  onUnauthorized(): void;
}

export function TrainerPanel({ client, agent, sessionActive, onUnauthorized }: Props) {
  const { state, submit, busy } = useCommand(client, onUnauthorized);
  const [target, setTarget] = useState('');
  const [targetError, setTargetError] = useState<string | null>(null);
  const online = agent?.online ?? false;
  const connected = online && agent?.connection_state != null && agent.connection_state !== 'idle' && agent.connection_state !== 'failed';
  const streaming = online && agent?.connection_state === 'streaming';
  const erg = streaming && agent?.erg_supported;
  const range = agent?.power_range_w;

  const applyTarget = (event: FormEvent) => {
    event.preventDefault();
    const watts = Number(target);
    const [min, max] = range ?? [0, 2000];
    if (!Number.isInteger(watts) || watts < min || watts > max) {
      setTargetError(`Hedef için ${min}–${max} W arasında tam sayı gir.`);
      return;
    }
    setTargetError(null);
    void submit('set_target_power', watts);
  };

  return (
    <section className="trainer-panel" aria-label="Trainer kontrolü">
      <div className="panel-row">
        <div>
          <h2>Trainer</h2>
          <p role="status">
            {!online
              ? 'Trainer ajanı çalışmıyor · başlatıcıyı --trainer ile ya da masaüstü uygulamasından aç.'
              : STATES[agent?.connection_state ?? 'idle'] ?? 'Durum bilinmiyor'}
          </p>
        </div>
        <div className="panel-actions">
          {connected ? (
            <button type="button" className="secondary" disabled={busy} onClick={() => void submit('disconnect')}>
              Bağlantıyı kes
            </button>
          ) : (
            <button type="button" disabled={busy || !online} onClick={() => void submit('connect')}>
              Trainer’a bağlan
            </button>
          )}
          {sessionActive || agent?.session_id ? (
            <button type="button" className="secondary" disabled={busy || !online} onClick={() => void submit('finish_session')}>
              Oturumu bitir
            </button>
          ) : (
            <button type="button" disabled={busy || !online} onClick={() => void submit('start_session')}>
              Oturumu başlat
            </button>
          )}
        </div>
      </div>
      <div className="panel-row erg">
        <div>
          <h3>Hedef güç (ERG)</h3>
          <p>
            {erg
              ? `Trainer direnci hedef güce göre ayarlar${range ? ` · desteklenen ${range[0]}–${range[1]} W` : ''}. ${
                  agent?.target_power_w != null ? `Şu anki hedef ${agent.target_power_w} W.` : 'Hedef ayarlı değil.'
                }`
              : 'Trainer bağlı ve hedef gücü destekliyor olduğunda kullanılabilir. Yeniden bağlantıdan sonra hedef otomatik gönderilmez.'}
          </p>
          {targetError && <p className="access-error">{targetError}</p>}
        </div>
        <form onSubmit={applyTarget} noValidate>
          <label htmlFor="target">
            Hedef <span>(W)</span>
          </label>
          <input
            id="target"
            type="number"
            step={1}
            min={range?.[0] ?? 0}
            max={range?.[1] ?? 2000}
            value={target}
            disabled={!erg}
            onChange={(event) => setTarget(event.target.value)}
          />
          <button type="submit" disabled={!erg || busy}>
            Uygula
          </button>
          <button
            type="button"
            className="text-button"
            disabled={!erg || busy || agent?.target_power_w == null}
            onClick={() => void submit('release_control')}
          >
            Direnci bırak
          </button>
        </form>
      </div>
      <p className="command-status" role="status">
        {describeCommand(state)}
      </p>
    </section>
  );
}
