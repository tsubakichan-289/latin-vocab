"""Rebuild the bundled seed from Wiktionary. Requires beautifulsoup4; caches HTML."""
import concurrent.futures
import datetime
import json
from pathlib import Path
import time
import unicodedata
import urllib.parse
import urllib.request
from bs4 import BeautifulSoup
from verb_data import compress_verb

ROOT = Path(__file__).resolve().parents[1]
CACHE = ROOT / 'data/cache'
SOURCE = 'https://en.wiktionary.org/wiki/Appendix:Latin_Swadesh_list'
JA = '''私|あなた（単数）|彼・彼女・それ|私たち|あなたたち|彼ら・彼女ら・それら|これ|あれ|ここ|そこ|誰|何|どこ|いつ|どのように|〜でない|すべての|多くの|いくらかの|少数の|ほかの|一つの|二つの|三つの|四つの|五つの|大きい|長い|広い|厚い|重い|小さい|短い|狭い|細い|女性|男性|人間|子ども|妻|夫|母|父|動物|魚|鳥|犬|シラミ|蛇|虫|木|森|棒|果実|種|葉|根|樹皮|花|草|縄|皮膚|肉|血|骨|脂肪|卵|角|尾|羽毛|毛|頭|耳|目|鼻|口|歯|舌|爪|足|脚|膝|手|翼|腹|内臓|首|背中|乳房|心臓|肝臓|飲む|食べる|噛む|吸う|唾を吐く|吐く|吹く|呼吸する|笑う|見る|聞く|知る|考える|においを嗅ぐ|恐れる|眠る|生きる|死ぬ|殺す|戦う|狩る|打つ|切る|割る|刺す|引っかく|掘る|泳ぐ|飛ぶ|歩く|来る|横たわる|座る|立つ|向きを変える|落ちる|与える|持つ|絞る|こする|洗う|拭く|引く|押す|投げる|結ぶ|縫う|数える|言う|歌う|遊ぶ|浮く|流れる|凍る|膨らむ|太陽|月|星|水|雨|川|湖|海|塩|石|砂|ほこり|土・大地|雲|霧|空|風|雪|氷|煙|火|灰|燃やす|道|山|赤い|緑の|黄色い|白い|黒い|夜|日|年|暖かい|冷たい|満ちた|新しい|古い|よい|悪い|腐った|汚い|まっすぐな|丸い|鋭い|鈍い|なめらかな|濡れた|乾いた|正しい|近い|遠い|右の|左の|〜で・〜に|〜の中に|〜とともに|そして|もし|〜なので|名前'''.split('|')
# Source forms are retained separately; dictionary entries are singular/base forms.
OVERRIDES = {1:'ego',4:'ego',5:'tū',6:'is',12:'quis',17:'omnis',18:'multus',20:'paucus',86:'viscus',111:'lūctor'}
INVARIABLE = {9,10,13,14,15,16,25,26,201,202,203,204,205,206}

def plain(s):
    return ''.join(c for c in unicodedata.normalize('NFD', s) if not unicodedata.combining(c))

def fetch(url):
    CACHE.mkdir(parents=True, exist_ok=True)
    path = CACHE / (urllib.parse.quote(url.rsplit('/', 1)[-1], safe='') + '.html')
    if path.exists():
        return path.read_text()
    for attempt in range(4):
        try:
            request = urllib.request.Request(url, headers={'User-Agent':'LatinVocab/0.1 (educational vocabulary seed builder)'})
            with urllib.request.urlopen(request, timeout=40) as response:
                html = response.read().decode('utf-8')
            path.write_text(html)
            time.sleep(0.4)
            return html
        except Exception:
            if attempt == 3:
                raise
            time.sleep(2 ** attempt)

def table_grid(table):
    occupied = {}
    rows = []
    for y, tr in enumerate(table.find_all('tr')):
        if tr.find_parent('table') is not table:
            continue
        x = 0
        for cell in tr.find_all(['td', 'th'], recursive=False):
            while (y,x) in occupied:
                x += 1
            value = ' '.join(cell.get_text(' ', strip=True).split())
            width, height = int(cell.get('colspan', 1)), int(cell.get('rowspan', 1))
            for dy in range(height):
                for dx in range(width):
                    occupied[y+dy,x+dx] = value
            x += width
        rows.append(y)
    width = max((x for y,x in occupied), default=-1) + 1
    return [[occupied.get((y,x), '') for x in range(width)] for y in rows]

def entry(item):
    url = 'https://en.wiktionary.org/wiki/' + urllib.parse.quote(plain(item['lemma']))
    item.update(source=url+'#Latin', tables=[], principal_parts='', pos='', morphology_status='missing')
    soup = BeautifulSoup(fetch(url), 'lxml')
    heading = soup.find(id='Latin')
    if heading is None:
        raise ValueError('No Latin section: '+url)
    groups = []
    current = None
    pos = ''
    for node in heading.parent.next_siblings:
        if not getattr(node, 'name', None):
            continue
        if node.select_one('h2'):
            break
        h = node.select_one('h3,h4,h5')
        if h and h.get_text() in {'Noun','Verb','Adjective','Pronoun','Adverb','Numeral','Preposition','Conjunction','Determiner','Particle','Participle'}:
            pos = h.get_text()
            current = None
        head = node.select_one('.headword[lang="la"]')
        if head:
            current = {'lemma':head.get_text(), 'pos':pos, 'principal_parts':node.get_text(' ', strip=True), 'tables':[]}
            groups.append(current)
        if current is not None:
            for table in ([node] if node.name == 'table' and set(node.get('class', [])) & {'inflection-table', 'roa-inflection-table'} else []) + node.select('table.inflection-table, table.roa-inflection-table'):
                grid = table_grid(table)
                if grid:
                    current['tables'].append(grid)
    exact = [g for g in groups if g['lemma'] == item['lemma']]
    matches = exact or [g for g in groups if plain(g['lemma']) == plain(item['lemma'])]
    expected_pos = {9:'Adverb',10:'Adverb',13:'Adverb',14:'Conjunction',15:'Adverb',16:'Particle',201:'Preposition',202:'Preposition',203:'Preposition',204:'Conjunction',205:'Conjunction',206:'Conjunction'}.get(item['number'])
    if 36 <= item['number'] <= 91 or 147 <= item['number'] <= 168 or item['number'] in {170,171,177,178,179,207}:
        expected_pos = 'Noun'
    elif 92 <= item['number'] <= 146 or item['number'] == 169:
        expected_pos = 'Verb'
    elif 27 <= item['number'] <= 35 or 172 <= item['number'] <= 176 or 180 <= item['number'] <= 200:
        expected_pos = 'Participle' if item['number'] in {189,191,196} else 'Adjective'
    if expected_pos:
        matches = [g for g in matches if g['pos'] == expected_pos]
    # volō 'fly' and volō 'want' share spelling but have distinct conjugations.
    if item['number'] == 120:
        matches = [g for g in matches if 'volāre' in g['principal_parts']]
    with_tables = [g for g in matches if g['tables']]
    if with_tables:
        # Preserve separate paradigms, but never blend different macron headwords (os / ōs).
        group = with_tables[0]
        item.update({k:group[k] for k in ('principal_parts','pos','tables')})
        item['morphology_status'] = 'imported'
    elif matches:
        item.update({k:matches[0][k] for k in ('principal_parts','pos')})
    if item['number'] in INVARIABLE:
        item['morphology_status'] = 'invariable'
    from classical_profile import apply_profile
    item = apply_profile(compress_verb(item))
    print(f"{item['number']:3} {item['lemma']}: {item['morphology_status']}", flush=True)
    return item

def main():
    assert len(JA) == 207
    soup = BeautifulSoup(fetch(SOURCE), 'lxml')
    items = []
    for tr in soup.select('tr'):
        cells = tr.find_all(['td','th'], recursive=False)
        if len(cells) < 3 or not cells[0].get_text(strip=True).isdigit():
            continue
        number = int(cells[0].get_text(strip=True))
        if not 1 <= number <= 207:
            continue
        latin = cells[2].select_one('[lang="la"]') or cells[2].find('a')
        if latin is None:
            raise ValueError(f'Missing Latin word: {number}')
        source_form = latin.get_text(strip=True)
        items.append(dict(number=number, lemma=OVERRIDES.get(number,source_form), source_form=source_form, meaning_ja=JA[number-1], concept_en=cells[1].get_text(' ',strip=True)))
    assert [i['number'] for i in items] == list(range(1,208))
    with concurrent.futures.ThreadPoolExecutor(max_workers=3) as pool:
        words = list(pool.map(entry, items))
    result = dict(source=SOURCE, retrieved=datetime.date.today().isoformat(), license='CC-BY-SA-4.0', morphology_edition='classical-school-1', words=words)
    target = ROOT / 'data/swadesh.json'
    temp = target.with_suffix('.tmp')
    temp.write_text(json.dumps(result, ensure_ascii=False, indent=2)+'\n')
    temp.replace(target)
    print('Saved', len(words), 'concepts;', sum(bool(w['tables']) for w in words), 'with tables')

if __name__ == '__main__':
    main()
