"""Manage an isolated development PostgreSQL cluster; never touches system clusters."""
import argparse
import getpass
from pathlib import Path
import secrets
import subprocess
from dotenv import dotenv_values, set_key

ROOT = Path(__file__).resolve().parents[1]
RUNTIME = ROOT / '.wattlab'
DATA = RUNTIME / 'postgres'
PORT = '55432'


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('action', choices=('start', 'stop', 'status'))
    options = parser.parse_args()
    bindir = Path(subprocess.check_output(['pg_config', '--bindir'], text=True).strip())
    pgctl = [str(bindir / 'pg_ctl'), '-D', str(DATA)]
    if options.action in ('stop', 'status'):
        subprocess.run(pgctl + [options.action], check=True)
        return
    RUNTIME.mkdir(mode=0o700, exist_ok=True)
    RUNTIME.chmod(0o700)
    if not (DATA / 'PG_VERSION').exists():
        subprocess.run([str(bindir / 'initdb'), '-D', str(DATA), '--auth-local=peer',
                        '--auth-host=scram-sha-256', '--no-instructions'], check=True)
    if subprocess.run(pgctl + ['status'], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL).returncode:
        subprocess.run(pgctl + ['-l', str(RUNTIME / 'postgres.log'), '-o',
                                f'-h 127.0.0.1 -p {PORT} -k {RUNTIME}', 'start'], check=True)
    env_path = ROOT / '.env'
    settings = dotenv_values(env_path)
    # The helper owns only these dedicated development database names.
    if settings.get('WATTLAB_DB_URL') not in (None, '', f'jdbc:postgresql://127.0.0.1:{PORT}/wattlab_dev', f'postgresql://127.0.0.1:{PORT}/wattlab_rust_dev'):
        raise SystemExit('Existing database configuration is custom; leave it unchanged and configure manually.')
    password = settings.get('WATTLAB_DB_PASSWORD') or secrets.token_urlsafe(32)
    if not all(c.isascii() and (c.isalnum() or c in '_-') for c in password):
        raise SystemExit('Local helper requires a URL-safe development password.')
    psql = [str(bindir / 'psql'), '-h', str(RUNTIME), '-p', PORT,
            '-U', getpass.getuser(), '-d', 'postgres', '-v', 'ON_ERROR_STOP=1', '-At']

    def exists(sql):
        return subprocess.check_output(psql + ['-c', sql], text=True).strip() == '1'

    if not exists("SELECT 1 FROM pg_roles WHERE rolname = 'wattlab_dev'"):
        subprocess.run(psql, input=f"CREATE ROLE wattlab_dev LOGIN PASSWORD '{password}';", text=True,
                       check=True, stdout=subprocess.DEVNULL)
    for name in ('wattlab_rust_dev', 'wattlab_rust_test'):
        if not exists(f"SELECT 1 FROM pg_database WHERE datname = '{name}'"):
            subprocess.run(psql, input=f'CREATE DATABASE {name} OWNER wattlab_dev;', text=True,
                           check=True, stdout=subprocess.DEVNULL)
    values = {
        'WATTLAB_API_URL': settings.get('WATTLAB_API_URL') or 'http://127.0.0.1:8080',
        'WATTLAB_API_TOKEN': settings.get('WATTLAB_API_TOKEN') or secrets.token_urlsafe(32),
        'WATTLAB_DB_URL': f'postgresql://127.0.0.1:{PORT}/wattlab_rust_dev',
        'WATTLAB_DB_USER': 'wattlab_dev', 'WATTLAB_DB_PASSWORD': password,
        'WATTLAB_TEST_DB_URL': f'postgresql://127.0.0.1:{PORT}/wattlab_rust_test',
        'WATTLAB_TEST_DB_USER': 'wattlab_dev', 'WATTLAB_TEST_DB_PASSWORD': password,
    }
    for key, value in values.items():
        set_key(env_path, key, value)
    env_path.chmod(0o600)
    print('Local database ready. Configuration is stored in ignored .env; no credentials displayed.')


if __name__ == '__main__':
    main()
