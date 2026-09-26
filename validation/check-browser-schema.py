"""Validate all version-1 message shapes with a Draft 2020-12 validator."""
import json
from pathlib import Path
from jsonschema import Draft202012Validator

schema = json.loads((Path(__file__).resolve().parents[1] / 'contracts/native-messaging.schema.json').read_text())
Draft202012Validator.check_schema(schema)
validator = Draft202012Validator(schema)
payloads = {
    'hello': dict(token='a'*43, extension_id='focus@deepify.local', browser_kind='firefox', profile_label='Fixture', capabilities=['top_level_web_request', 'tab_restore', 'container_tabs']),
    'pair': dict(token='a'*43),
    'pair_result': dict(accepted=True, request_id='request'),
    'heartbeat': {}, 'heartbeat_ack': dict(request_id='request'), 'status': {},
    'start_session': dict(session_id='session', timer_state='working', remaining_seconds=60, rules=[dict(host='example.org',path='/Docs/')]),
    'start_result': dict(session_id='session', accepted=True, request_id='request'),
    'stop_session': dict(session_id='session'),
    'stop_result': dict(session_id='session', accepted=True, request_id='request'),
    'state': dict(health='active', session_id='session', timer_state='paused', remaining_seconds=60),
    'blocked_attempt': dict(session_id='session'),
    'integration_error': dict(error_code='internal'),
    'restore_error': dict(error_code='restore_failed', session_id='session'),
    'restore_complete': dict(session_id='session', restored_count=1),
}
assert set(payloads) == set(schema['properties']['type']['enum'])
checks = 0
for kind, payload in payloads.items():
    message = dict(version=1, type=kind, message_id='id', **payload)
    validator.validate(message)
    for key in ('version', 'type', 'message_id'):
        invalid = {k:v for k,v in message.items() if k != key}
        assert not validator.is_valid(invalid), (kind, key)
        checks += 1
    for extra in (dict(url='https://private.invalid'), dict(message_id=''), dict(message_id='a'*129), dict(message_id='control\u0085character'), dict(version=2), dict(request_id='a'*129)):
        assert not validator.is_valid(message | extra), (kind, extra)
        checks += 1
    # Every message type must reject fields valid only in a different payload.
    for key in set(schema['properties']) - set(message) - {'request_id'}:
        if key not in next(item['then']['propertyNames']['enum'] for item in schema['allOf'] if item['if']['properties']['type'].get('const') == kind):
            assert not validator.is_valid(message | {key:None}), (kind, key)
            checks += 1
for kind in ('pair_result','heartbeat_ack','start_result','stop_result'):
    message = dict(version=1,type=kind,message_id='id',**payloads[kind]); del message['request_id']
    assert not validator.is_valid(message)
    checks += 1
print(f'PASS: Draft 2020-12 schema; {len(payloads)} message types and {checks} rejection checks')
