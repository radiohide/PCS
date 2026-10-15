"""Read an immutable EVTX using both plugins; never ingest into the running database.
Usage: python compare.py <bundle-directory> <archive.evtx> [channel]
Run as the same Windows user as PCScope (DPAPI needs a loaded user profile).
"""
import json
import pathlib
import subprocess
import sys
import time

root = pathlib.Path(sys.argv[1]).resolve()
archive = pathlib.Path(sys.argv[2]).resolve()
channel = sys.argv[3] if len(sys.argv) > 3 else 'Microsoft-Windows-PowerShell/Operational'
request = {'channel': channel, 'archive_path': str(archive), 'watch': {'files': []}}
config = root / 'eventlog-backend.json'
assert not config.exists() or json.loads(config.read_text())['backend'] == 'powershell', 'Comparison requires the wrapper to use PowerShell'


def call(exe, req):
    started = time.perf_counter()
    proc = subprocess.run([str(root / exe)], input=json.dumps(req), encoding='utf-8',
                          capture_output=True, timeout=60, check=True)
    result = json.loads(proc.stdout)
    assert result['type'] == 'event_batch', result.get('reason', result)
    return result, time.perf_counter() - started


def identity(event):
    data = event['data']
    return (event['id'], event['kind'], event['object_id'], data.get('xml_sha256'))


results = {}
for exe in ('pcscope-eventlog-native.exe', 'pcscope-eventlog.exe'):
    first, seconds = call(exe, request)
    assert first['events'], 'Empty archive cannot validate equivalence'
    cursor = json.loads(first['checkpoint'])
    resume = {'epoch': first['epoch'], 'batch_seq': first['batch_seq'], 'checkpoint': first['checkpoint']}
    second, resume_seconds = call(exe, dict(request, resume=resume))
    assert second['epoch'] == first['epoch']
    assert second['batch_seq'] == first['batch_seq'] + 1
    assert all(int(e['id']) > cursor['record_id'] for e in second['events'])
    # A changed fingerprint must explicitly mark a reset and re-read retained history.
    broken = dict(cursor, fingerprint='0' * 64)
    reset, _ = call(exe, dict(request, resume=dict(resume, checkpoint=json.dumps(broken))))
    assert reset['events'][0]['kind'] == 'coverage.log_reset'
    assert reset['epoch'] != first['epoch']
    assert identity(reset['events'][1]) == identity(first['events'][0])
    assert all(e['data']['raw_xml']['protection'] == 'dpapi-current-user-v1' for e in first['events'])
    for event in first['events']:
        fields = event['data']['fields']
        if 'ScriptBlockText' in fields:
            assert fields['ScriptBlockText']['sensitive'] is True
    results[exe] = first
    print(json.dumps({'plugin': exe, 'first_batch_seconds': round(seconds, 3),
                      'first_batch_events': len(first['events']), 'resume_seconds': round(resume_seconds, 3),
                      'resume_events': len(second['events']), 'resume_and_reset': 'passed',
                      'encrypted_xml_and_scripts': 'passed'}), flush=True)

a, b = [r['events'] for r in results.values()]
common = min(len(a), len(b))
assert [identity(e) for e in a[:common]] == [identity(e) for e in b[:common]]
for x, y in zip(a, b):
    assert x['data']['system'] == y['data']['system']
    assert x['data']['fields'].keys() == y['data']['fields'].keys()
    for key, value in x['data']['fields'].items():
        other = y['data']['fields'][key]
        if isinstance(value, str) and isinstance(other, str):
            assert value == other, key
print(json.dumps({'matching_event_ids_classification_xml_hashes_system_and_field_names': common}))
