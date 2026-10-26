import sys
import unittest
from pathlib import Path
sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from regression import assess
from soak import assess as memory_assess

class PerformancePolicy(unittest.TestCase):
    def test_reproducible_large_slowdown_is_flagged(self):
        self.assertTrue(assess([.2] * 7, [.1] * 7)['regression'])
    def test_single_outlier_does_not_fail(self):
        self.assertFalse(assess([.1] * 6 + [10], [.1] * 7)['regression'])
    def test_startup_noise_below_absolute_threshold_does_not_fail(self):
        self.assertFalse(assess([.004] * 7, [.001] * 7)['regression'])
    def test_small_change_is_not_a_regression(self):
        self.assertFalse(assess([.11] * 7, [.1] * 7)['regression'])
    def test_invalid_samples_fail_closed(self):
        for candidate, baseline in [([1] * 4, [1] * 4), ([0] * 5, [1] * 5), ([float('nan')] * 5, [1] * 5)]:
            with self.assertRaises(ValueError): assess(candidate, baseline)

class MemoryPolicy(unittest.TestCase):
    def test_warmup_is_not_counted_as_a_leak(self):
        samples = [(0, 1), (1, 8 * 2**20)] + [(x, 10 * 2**20) for x in range(2, 20)]
        self.assertTrue(memory_assess(samples, 2, 64, 8)['passed'])
    def test_growth_and_peak_limits_detect_leaks(self):
        samples = [(x, (10 + x) * 2**20) for x in range(20)]
        self.assertFalse(memory_assess(samples, 2, 64, 8)['passed'])
        self.assertFalse(memory_assess([(x, 70 * 2**20) for x in range(20)], 2, 64, 8)['passed'])
    def test_too_few_samples_cannot_pass(self):
        with self.assertRaises(ValueError): memory_assess([(0, 1)], 2, 64, 8)

if __name__ == '__main__': unittest.main()
