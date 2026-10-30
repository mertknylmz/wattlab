import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from trainer_scan import FTMS, device_entry, is_trainer, rank_devices


class ScanTests(unittest.TestCase):
    def test_only_trainers_survive_the_nearby_list(self):
        phone = device_entry("11:22:33:44:55:66", "Phone", -30)
        buds = device_entry("22:33:44:55:66:77", "Mertkaan's Buds2 Pro", -40)
        nameless = device_entry("33:44:55:66:77:88", None, -50)
        named = device_entry("AA:BB:CC:DD:EE:FF", "D500-V2", -60)
        advertised = device_entry("AB:CD:EF:01:23:45", "Indoor", -70, [FTMS])
        ranked = rank_devices([phone, buds, nameless, named, advertised])
        self.assertEqual([item["name"] for item in ranked], ["Indoor", "D500-V2"])
        self.assertTrue(is_trainer(named))
        self.assertFalse(is_trainer(buds))
        self.assertFalse(is_trainer(nameless))
