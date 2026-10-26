import json
from pathlib import Path
import sys
import unittest
sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from summarize_trace import summarize

class TraceSummary(unittest.TestCase):
    def event(self, operation, total, error=None):
        return 'progress text\rpv-trace: ' + json.dumps({'schema': 1, 'pid': 42, 'transfer': 1, 'operation': operation,
            'bytes': 7, 'total_bytes': total, 'elapsed_ns': 100, 'duration_ns': 10, 'error_kind': error})
    def test_aggregates_costs_and_errors_while_ignoring_progress(self):
        result = summarize(['unrelated text', self.event('write', 7), self.event('write', 14, 'BrokenPipe')])
        self.assertEqual(result['transfers']['42:1']['bytes'], 14)
        self.assertEqual(result['operations']['write']['calls'], 2)
        self.assertEqual(result['operations']['write']['duration_ns'], 20)
        self.assertEqual(result['operations']['write']['errors'], 1)
    def test_missing_malformed_and_unknown_schema_fail(self):
        for lines in [[], ['pv-trace: invalid'], ['pv-trace: {"schema": 2}']]:
            with self.assertRaises((ValueError, KeyError)): summarize(lines)
    def test_invalid_measurements_cannot_be_aggregated(self):
        text = self.event('write', -1)
        with self.assertRaises(ValueError): summarize([text])

if __name__ == '__main__': unittest.main()
