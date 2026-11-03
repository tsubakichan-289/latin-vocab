import copy
import json
import unittest
from pathlib import Path
from classical_profile import PROFILE, apply_profile
class ClassicalProfileTests(unittest.TestCase):
    def test_all_corrections_and_reimport_are_idempotent(self):
        for rule in PROFILE['rules']:
            word = dict(lemma=rule['lemma'], pos=rule['pos'])
            for field in ('tables', 'verb', 'principal_parts'):
                if rule['previous_'+field] is not None:
                    word[field] = copy.deepcopy(rule['previous_'+field])
            apply_profile(word)
            for field in ('tables', 'verb', 'principal_parts'):
                self.assertEqual(word.get(field),rule[field])
            self.assertEqual(apply_profile(copy.deepcopy(word)),word)
    def test_seed_obeys_profile(self):
        seed=json.loads((Path(__file__).resolve().parents[1]/'data/swadesh.json').read_text())
        for word in seed['words']:
            self.assertEqual(apply_profile(copy.deepcopy(word)),word)
