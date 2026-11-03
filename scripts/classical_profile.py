"""Apply the checked-in classical teaching profile after source extraction.

Unknown source changes fail explicitly instead of silently choosing a first form.
"""
import json
from pathlib import Path
PROFILE = json.loads((Path(__file__).resolve().parents[1] / 'data/classical_profile.json').read_text())
def apply_profile(word):
    rule = next((r for r in PROFILE['rules'] if r['lemma'] == word['lemma'] and r['pos'] == word['pos']), None)
    if rule:
        for field in ('tables', 'verb', 'principal_parts'):
            value = word.get(field)
            if value not in (rule['previous_' + field], rule[field]):
                raise ValueError(f"Classical profile needs review: {word['lemma']} / {field}")
            if rule[field] is not None:
                word[field] = rule[field]
    return word
