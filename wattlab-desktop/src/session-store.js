'use strict';
// Persist the member login token only when the OS can encrypt it. Linux
// basic_text (no keyring) is treated as unavailable so we never write plaintext.

const DEFAULT_NAME = 'session.bin';

function createSessionStore({ fs, path, safeStorage, file }) {
  const target = file;
  function available() {
    return (
      Boolean(safeStorage.isEncryptionAvailable()) &&
      safeStorage.getSelectedStorageBackend() !== 'basic_text'
    );
  }
  function clear() {
    try {
      fs.unlinkSync(target);
    } catch (error) {
      if (error.code !== 'ENOENT') throw error;
    }
  }
  return {
    available,
    load() {
      if (!available()) return null;
      try {
        return safeStorage.decryptString(fs.readFileSync(target));
      } catch {
        return null;
      }
    },
    save(token) {
      if (!available()) return false;
      fs.mkdirSync(path.dirname(target), { recursive: true });
      fs.writeFileSync(target, safeStorage.encryptString(token));
      return true;
    },
    clear,
  };
}

function sessionFile(userData, path) {
  return path.join(userData, DEFAULT_NAME);
}

module.exports = { createSessionStore, sessionFile };
