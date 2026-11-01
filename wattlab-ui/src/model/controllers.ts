import type { ControllerDevice, ControllerLink } from '../api/types';
import type { Translate } from '../i18n/appearance';
import type { MessageKey } from '../i18n/messages';

const FAILURES: Record<string, MessageKey> = {
  controller_connect_failed: 'reasonControllerConnectFailed',
  controller_connect_timeout: 'reasonControllerConnectTimeout',
};

export function controllerLabel(
  device: { name: string | null; address: string; kind: ControllerDevice['kind']; side: ControllerDevice['side'] },
  t: Translate,
): string {
  const name = device.name || device.address;
  const kind = device.kind === 'play' ? t('controllerPlay') : t('controllerClick');
  if (device.side === 'left') return `${name} · ${kind} · ${t('sideLeft')}`;
  if (device.side === 'right') return `${name} · ${kind} · ${t('sideRight')}`;
  return `${name} · ${kind}`;
}

function names(links: ControllerLink[], t: Translate): string {
  return links.map((link) => controllerLabel(link, t)).join(', ');
}

function failure(link: ControllerLink, t: Translate): string {
  const reason =
    link.message && FAILURES[link.message] ? t(FAILURES[link.message]) : t('reasonControllerConnectFailed');
  return `${controllerLabel(link, t)} (${reason})`;
}

/** One line for the Settings controller card. Links outrank the discovery list. */
export function controllerStatus(links: ControllerLink[], discovered: ControllerDevice[], t: Translate): string {
  const connecting = links.filter((link) => link.state === 'connecting');
  if (connecting.length) return t('controllerConnecting', { names: names(connecting, t) });
  const connected = links.filter((link) => link.state === 'connected');
  const failed = links.filter((link) => link.state === 'failed');
  if (connected.length && failed.length) {
    return t('controllerPartial', {
      connected: names(connected, t),
      failed: failed.map((link) => failure(link, t)).join(', '),
    });
  }
  if (connected.length) return t('controllerConnected', { names: names(connected, t) });
  if (failed.length) return t('controllerFailed', { names: failed.map((link) => failure(link, t)).join(', ') });
  if (links.some((link) => link.state === 'disconnected')) return t('controllerDisconnected');
  if (discovered.length) return t('controllerFound', { count: discovered.length });
  return t('controllerDisconnected');
}
