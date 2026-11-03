import copy
import unittest
from verb_data import compress_verb

class CompressionTests(unittest.TestCase):
    def word(self, lemma, infinitive, perfect, conjugation):
        return dict(lemma=lemma, pos='Verb', tables=[[['source table']]],
                    principal_parts=f'{lemma} ( present infinitive {infinitive} , perfect active {perfect} ) ; {conjugation}')

    def test_regular_verbs_discard_source_tables(self):
        word=compress_verb(self.word('regō','regere','rēxī','third conjugation'))
        self.assertEqual(word['tables'],[])
        self.assertEqual(word['verb']['conjugation'],'third')
        self.assertEqual(word['verb']['principal_parts']['perfect'],['rēxī'])
        self.assertEqual(word['verb']['principal_parts']['supine'],[])

    def test_homograph_storage_is_decided_by_infinitive(self):
        fly=compress_verb(self.word('volō','volāre','volāvī','first conjugation'))
        want=self.word('volō','velle','voluī','irregular conjugation')
        original=copy.deepcopy(want['tables'])
        want=compress_verb(want)
        self.assertEqual(fly['verb']['kind'],'regular')
        self.assertEqual(want['verb']['kind'],'irregular')
        self.assertEqual(want['tables'],original)

    def test_deponent_and_specific_exceptions_are_compact(self):
        die=compress_verb(self.word('morior','morī','mortuus sum','third ( -iō variant) conjugation , deponent'))
        self.assertTrue(die['verb']['deponent'])
        self.assertEqual(die['verb']['principal_parts']['perfect'],[])
        self.assertEqual(die['verb']['principal_parts']['perfect_participle'],'mortuus')
        self.assertEqual(die['verb']['future_participle_stem'],'morit')
        self.assertEqual(die['tables'],[])
        give=compress_verb(self.word('dō','dare','dedī','first conjugation'))
        self.assertEqual(give['verb']['overrides']['participle.active.present'],['dāns'])

    def test_nouns_are_unchanged_and_unknown_headers_fail(self):
        noun=dict(lemma='aqua',pos='Noun',tables=[[['aqua','aquae']]])
        original=copy.deepcopy(noun)
        self.assertEqual(compress_verb(noun),original)
        with self.assertRaises(ValueError):
            compress_verb(self.word('sum','esse','fuī','irregular conjugation') | {'tables':[]})
        with self.assertRaises(ValueError):
            compress_verb(self.word('unknown','unknown','unknown','unknown conjugation'))

if __name__=='__main__':
    unittest.main()
