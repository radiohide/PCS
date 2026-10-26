"""Destructive acceptance only inside a new target fixture; never uses saved app paths."""
import json
import os
from pathlib import Path
import shutil
import sqlite3
import subprocess
import time

ROOT = Path(__file__).resolve().parents[1]
TEST = ROOT / 'target' / ('history-reset-e2e-' + str(time.time_ns()))
TEST.mkdir()
DATA = TEST / 'data'
DATA.mkdir()
assert DATA.resolve().is_relative_to((ROOT / 'target').resolve())
for source, name in [('pcscope-service.exe', 'pcscope-service.exe'),
                     ('pcscope-cli.exe', 'pcscope-cli.exe'), ('examples/probe.exe', 'probe.exe')]:
    shutil.copyfile(ROOT / 'target/release' / source, TEST / name)
    subprocess.run(['icacls.exe', str(TEST / name), '/setintegritylevel', 'M'], check=True, capture_output=True)
ENV = {**os.environ, 'PCSCOPE_DATA_DIR': str(DATA)}
FLAGS = subprocess.CREATE_NO_WINDOW


def request(operation, **args):
    result = subprocess.run([str(TEST / 'probe.exe'), 'request', json.dumps({'operation': operation, **args})],
                            env=ENV, capture_output=True, text=True, encoding='utf-8', timeout=15, creationflags=FLAGS)
    if result.returncode:
        raise RuntimeError(result.stderr)
    return json.loads(result.stdout)


def wait_ready():
    until = time.monotonic() + 90
    while time.monotonic() < until:
        try:
            return request('retention')
        except (RuntimeError, subprocess.TimeoutExpired):
            time.sleep(.5)
    raise AssertionError('fixture daemon did not become ready')


subprocess.run([str(TEST / 'pcscope-cli.exe'), 'init-baseline', str(DATA / 'history.db'), '600'],
               check=True, capture_output=True, creationflags=FLAGS)
ids = ['pcscope-software.exe', 'pcscope-powershell-adapter.exe', 'system', 'security', 'network',
       'storage', 'startup', 'desktop', 'virtualization', 'applications', 'scoped', 'eventlog', 'usn']
settings = {
    'plugin_schedule': json.dumps({**{key: {'enabled': False, 'seconds': 600} for key in ids},
                                  '_global_seconds': 600, '_runtime_seconds': 600}),
    'runtime_watch': json.dumps({'enabled': False, 'module_process_names': ['sihost'], 'revision': 7}),
    'monitoring': json.dumps({'mode': 'low', 'repair': False, 'configured': True}),
    'file_watch': json.dumps({'files': [], 'revision': 3}),
    'directory_watch': json.dumps({'directories': [], 'revision': 4}),
    'item_settings': json.dumps({'disabled': ['pcscope.network.connections'], 'revision': 6}),
    'show_sensitive': 'true',
}
with sqlite3.connect(DATA / 'history.db') as db:
    db.executemany('INSERT OR REPLACE INTO settings VALUES(?,?)', settings.items())
    db.executescript('CREATE TABLE deletion_fixture(data BLOB); INSERT INTO deletion_fixture VALUES(zeroblob(8000000));')
db.close()
started = subprocess.Popen([str(TEST / 'pcscope-service.exe'), str(DATA)], env=ENV, creationflags=FLAGS,
                           stdout=(TEST / 'stdout.log').open('w'), stderr=(TEST / 'stderr.log').open('w'))
try:
    before = wait_ready()
    assert all(not job['enabled'] for job in request('plugins')), 'fixture must not collect'
    began = time.monotonic()
    receipt = request('delete_history', history_id=before['history_id'], confirmation='DELETE ALL HISTORY')
    assert receipt['state'] == 'pending'
    until = began + 90
    while time.monotonic() < until:
        try:
            result = request('history_deletion_status')
            if result['state'] == 'completed':
                break
            assert result['state'] == 'pending', result
        except (RuntimeError, subprocess.TimeoutExpired):
            pass
        time.sleep(.5)
    else:
        raise AssertionError('reset did not complete')
    after = wait_ready()
    assert after['history_id'] != before['history_id']
    assert after['physical_bytes'] < before['physical_bytes'] / 2
    with sqlite3.connect(DATA / 'history.db') as db:
        current = dict(db.execute('SELECT key,value FROM settings'))
        assert all(current.get(key) == value for key, value in settings.items()), current
        assert not db.execute("SELECT 1 FROM sqlite_master WHERE name='deletion_fixture'").fetchone()
        assert current['history_reset_operation'] == receipt['id']
    db.close()
    stale_rejected = False
    try:
        request('delete_history', history_id=before['history_id'], confirmation='DELETE ALL HISTORY')
    except RuntimeError:
        stale_rejected = True
    assert stale_rejected
    report = {'passed': True, 'seconds': round(time.monotonic() - began, 3),
              'before_bytes': before['physical_bytes'], 'after_bytes': after['physical_bytes'],
              'settings_preserved': list(settings), 'fixture': str(DATA)}
    (TEST / 'result.json').write_text(json.dumps(report, indent=2), encoding='utf-8')
    print(json.dumps(report))
finally:
    try:
        request('shutdown')
    except (RuntimeError, subprocess.TimeoutExpired):
        pass
    started.wait(timeout=30)
