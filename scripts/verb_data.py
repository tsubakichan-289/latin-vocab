"""Convert verb dictionary headers to compact data. Never persist regular tables."""
import re

STORED_IRREGULARS = {
    ('sum', 'esse'), ('possum', 'posse'), ('ferō', 'ferre'), ('eō', 'īre'),
    ('volō', 'velle'), ('nōlō', 'nōlle'), ('mālō', 'mālle'), ('fīō', 'fierī'),
}


def finite_cells(tables):
    """Read the six person columns from Wiktionary's expanded colspan grid."""
    for table in tables:
        mood = None
        for row in table:
            if row[0] in {'indicative', 'subjunctive', 'imperative'}:
                mood = row[0]
            elif row[0] in {'active', 'passive'} and mood:
                if len(row) < 8 or (len(row)-2) % 6:
                    raise ValueError('Unexpected verb table width')
                step = (len(row)-2)//6
                for i, person in enumerate(('1s','2s','3s','1p','2p','3p')):
                    value = row[2+i*step]
                    forms = [] if value == '—' else [s.strip() for s in value.split(',')]
                    yield f'{mood}.{row[0]}.{row[1].replace(" ", "_")}.{person}', forms
            else:
                mood = None


def compress_verb(word):
    if word['pos'] != 'Verb':
        return word
    text = word['principal_parts'].replace('\u0306','')
    match = re.search(r'present infinitive (.*?) , perfect active (.*?)(?: , supine (.*?))? \)', text)
    if not match:
        raise ValueError('Cannot parse principal parts: '+text)
    infinitive, perfect, supine = match.groups()
    infinitive = infinitive.split(' or ')[0]
    if (word['lemma'],infinitive) in STORED_IRREGULARS:
        if not word['tables']:
            raise ValueError('An important irregular verb requires a stored table')
        word['verb'] = {'kind':'irregular', 'infinitive':infinitive}
        return word
    if 'first conjugation' in text:
        conjugation = 'first'
    elif 'second conjugation' in text:
        conjugation = 'second'
    elif 'third ( -iō variant)' in text:
        conjugation = 'third_io'
    elif 'third conjugation' in text:
        conjugation = 'third'
    elif 'fourth conjugation' in text:
        conjugation = 'fourth'
    else:
        raise ValueError('Unknown conjugation: '+text)
    deponent = 'deponent' in text
    parts = dict(present=word['lemma'], infinitive=infinitive,
                 perfect=[] if deponent else perfect.split(' or '),
                 supine=supine.split(' or ') if supine else [])
    if deponent:
        if not perfect.endswith(' sum'):
            raise ValueError('Unexpected deponent principal part')
        parts['perfect_participle'] = perfect.removesuffix(' sum')
    verb = dict(kind='regular',principal_parts=parts,conjugation=conjugation)
    if deponent:
        verb['deponent'] = True
        if word['lemma']=='morior':
            verb['passive'] = 'impersonal'
            verb['future_participle_stem'] = 'morit'
    elif 'no passive' in text:
        verb['passive'] = 'none'
    elif 'impersonal in the passive' in text:
        verb['passive'] = 'impersonal'
    if word['lemma']=='dō':
        verb['short_a'] = True
    if word['lemma']=='stō':
        verb['future_participle_stem'] = 'stāt'
    overrides = {}
    if word['lemma']=='dō':
        overrides['participle.active.present'] = ['dāns']
    if word['lemma']=='morior':
        # Keep the source dictionary's missing gerundive explicit.
        overrides['participle.passive.future'] = []
    if word['lemma']=='dīcō':
        overrides['imperative.active.present.2s'] = ['dīc']
    if word['lemma']=='edō':
        for key, forms in finite_cells(word['tables']):
            if any(f.startswith('ēs') for f in forms):
                overrides[key] = forms
        overrides['infinitive.active.present'] = ['edere','ēsse']
    if word['lemma']=='olfaciō':
        for key, forms in finite_cells(word['tables']):
            mood, voice, tense, person = key.split('.')
            if voice=='passive' and tense in {'present','imperfect','future'}:
                overrides[key] = forms
        overrides['infinitive.passive.present'] = ['olfierī']
    if overrides:
        verb['overrides'] = overrides
    word['verb'] = verb
    word['tables'] = []
    word['morphology_status'] = 'generated'
    return word
