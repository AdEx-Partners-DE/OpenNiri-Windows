"""Guard this repository's deliberately small CI contract (stdlib only).

These structural checks complement GitHub's YAML/workflow validation. They are
not a general YAML parser and must be updated explicitly if its layout changes.
They never run the window manager or claim host-acceptance evidence.
"""
from pathlib import Path
import re
import unittest

ROOT = Path(__file__).resolve().parents[2]
WORKFLOW = ROOT / '.github/workflows/ci.yml'


def violations(text):
    errors = []
    trigger = re.search(r'(?ms)^on:\n(.*?)(?=^\S)', text)
    events = trigger.group(1) if trigger else ''
    for event in ('push', 'pull_request'):
        block = re.search(rf'(?ms)^  {event}:\n(.*?)(?=^  \S|\Z)', events)
        if not block or '    branches: [main]\n' not in block.group(1):
            errors.append(f'{event}: main must be covered')
    if "    tags: ['**']\n" not in events:
        errors.append('all existing tag builds must remain covered')
    if '  workflow_dispatch:\n' not in events:
        errors.append('manual exact-ref verification must remain available')
    if 'permissions:\n  contents: read\n' not in text:
        errors.append('default token permissions must be read-only')
    actions = re.findall(r'^\s+uses: (\S+)', text, re.M)
    if not actions or any(not re.fullmatch(r'[\w.-]+/[\w./-]+@[a-f0-9]{40}', a) for a in actions):
        errors.append('external actions require full commit pins')
    checkouts = re.findall(r'(?ms)^      - name: Checkout\n(.*?)(?=^      -|\Z)', text)
    if len(checkouts) != 2 or any('          persist-credentials: false\n' not in s for s in checkouts):
        errors.append('both checkouts must avoid persisted credentials')
    commands = re.findall(r'^\s+run: (cargo (?:build|test|clippy) .+)$', text, re.M)
    if len(commands) != 7 or any('--locked' not in c.split() for c in commands):
        errors.append('all seven dependency-resolving build/test/lint commands must be locked')
    if '        run: python tools/ci/test_workflow_contract.py\n' not in text:
        errors.append('CI must execute this contract')
    release = re.search(r'(?ms)^  release:\n(.*)', text)
    if not release or not re.search(r"^    if: startsWith\(github.ref, 'refs/tags/v'\)$", release.group(1), re.M):
        errors.append('release must remain version-tag-only')
    if '    needs: build\n' not in (release.group(1) if release else ''):
        errors.append('release must depend on a green build')
    if text.count('- name: Enforce INC-49 release closure gate') != 2:
        errors.append('both incident gates must remain present')
    for marker in ('"INC-49-1", "INC-49-4"', '"INC-49-T1"', '$task.status -ne "done"', '$requiredHostTest.status -ne "done"'):
        if text.count(marker) != 2:
            errors.append(f'incident gate condition missing: {marker}')
    return errors


class WorkflowContract(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.text = WORKFLOW.read_text(encoding='utf-8')

    def test_current_contract(self):
        self.assertEqual(violations(self.text), [])

    def test_missing_main_push_is_rejected(self):
        self.assertTrue(violations(self.text.replace('    branches: [main]\n', '', 1)))

    def test_missing_manual_trigger_is_rejected(self):
        self.assertTrue(violations(self.text.replace('  workflow_dispatch:\n', '')))

    def test_moving_action_pin_is_rejected(self):
        self.assertTrue(violations(re.sub(r'actions/checkout@[a-f0-9]{40}', 'actions/checkout@v7', self.text)))

    def test_persisted_credentials_are_rejected(self):
        self.assertTrue(violations(self.text.replace('persist-credentials: false', 'persist-credentials: true')))

    def test_unlocked_build_is_rejected(self):
        self.assertTrue(violations(self.text.replace('cargo build --locked', 'cargo build', 1)))

    def test_main_release_is_rejected(self):
        self.assertTrue(violations(self.text.replace("    if: startsWith(github.ref, 'refs/tags/v')", "    if: github.ref == 'refs/heads/main'")))

    def test_weakened_incident_gate_is_rejected(self):
        self.assertTrue(violations(self.text.replace('$task.status -ne "done"', '$false', 1)))

    def test_elevated_default_permissions_are_rejected(self):
        self.assertTrue(violations(self.text.replace('permissions:\n  contents: read', 'permissions:\n  contents: write')))


if __name__ == '__main__':
    unittest.main()
