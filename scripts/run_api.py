"""Run or test WattLab API using Cargo. The active backend is Rust."""
import argparse
import os
from pathlib import Path
import shutil

from dotenv import load_dotenv


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    mode = parser.add_mutually_exclusive_group()
    mode.add_argument('--test', action='store_true', help='Run hardware/database-free Rust tests')
    mode.add_argument('--integration', action='store_true', help='Run opt-in PostgreSQL integration tests')
    parser.add_argument('--offline', action='store_true', help='Use already downloaded dependencies')
    options = parser.parse_args()
    root = Path(__file__).resolve().parents[1]
    load_dotenv(root / '.env')
    cargo = shutil.which('cargo') or str(Path.home() / '.cargo' / 'bin' / 'cargo')
    if not Path(cargo).is_file():
        parser.error('Cargo not found; install Rustup and restart the terminal')
    command = [cargo, 'test' if options.test or options.integration else 'run', '--locked']
    if options.offline:
        command.append('--offline')
    if options.integration:
        command.extend(['--test', 'postgres', '--', '--ignored'])
    os.chdir(root / 'wattlab-api')
    os.execvpe(cargo, command, os.environ)


if __name__ == '__main__':
    main()
