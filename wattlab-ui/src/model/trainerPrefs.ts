const ADDRESS_KEY = 'wattlab.trainerAddress';
const NAME_KEY = 'wattlab.trainerName';

const ADDRESS = /^([0-9A-F]{2}:){5}[0-9A-F]{2}$/;

export interface LastTrainer {
  address: string;
  name: string | null;
}

export function readTrainerAddress(): string | null {
  return readLastTrainer()?.address ?? null;
}

export function readLastTrainer(): LastTrainer | null {
  try {
    const address = localStorage.getItem(ADDRESS_KEY);
    if (!address || !ADDRESS.test(address)) return null;
    const name = localStorage.getItem(NAME_KEY);
    return { address, name: name && name.trim() ? name : null };
  } catch {
    return null;
  }
}

export function writeTrainerAddress(address: string, name: string | null = null) {
  try {
    localStorage.setItem(ADDRESS_KEY, address);
    if (name && name.trim()) localStorage.setItem(NAME_KEY, name.trim());
    else localStorage.removeItem(NAME_KEY);
  } catch {
    /* private mode */
  }
}

export function trainerLabel(trainer: LastTrainer): string {
  return trainer.name || trainer.address;
}
