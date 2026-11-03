//! Range selection for copy practice; all paradigms stay in memory.
use latin_vocab::{Word, morphology::Table};
use std::collections::BTreeSet;
pub const TENSES: [&str; 6] = ["現在", "未完了過去", "未来", "完了", "過去完了", "未来完了"];
pub const MOODS: [&str; 3] = ["直説法", "接続法", "命令法"];
#[derive(Clone)]
pub struct Range {
    pub nouns: bool,
    pub verbs: bool,
    pub declensions: [bool; 5],
    pub tenses: [bool; 6],
    pub moods: [bool; 3],
    pub voices: [bool; 2],
    pub blanks: usize,
}
impl Default for Range {
    fn default() -> Self {
        Self {
            nouns: true,
            verbs: true,
            declensions: [true; 5],
            tenses: [true; 6],
            moods: [true; 3],
            voices: [true; 2],
            blanks: 4,
        }
    }
}
impl Range {
    pub fn accepts(&self, w: &Word) -> bool {
        if w.pos == "Noun" && self.nouns {
            ["first", "second", "third", "fourth", "fifth"]
                .iter()
                .enumerate()
                .any(|(i, n)| {
                    self.declensions[i] && w.principal_parts.contains(&format!("{n} declension"))
                })
        } else {
            w.pos == "Verb" && self.verbs
        }
    }
    pub fn tables(&self, w: &Word) -> latin_vocab::Result<Vec<Table>> {
        if !self.accepts(w) {
            return Ok(vec![]);
        }
        let tables = w.display_tables()?;
        if w.pos == "Noun" {
            return Ok(tables);
        }
        Ok(tables
            .into_iter()
            .filter_map(|t| {
                let heading = t.first()?.first()?;
                if !MOODS
                    .iter()
                    .enumerate()
                    .any(|(i, m)| self.moods[i] && heading.starts_with(m))
                {
                    return None;
                }
                let passive = heading.contains("受動態");
                if !self.voices[usize::from(passive)] {
                    return None;
                }
                let tense = heading.rsplit(" / ").next()?;
                TENSES
                    .iter()
                    .enumerate()
                    .any(|(i, t)| self.tenses[i] && tense == *t)
                    .then_some(t)
            })
            .collect())
    }
    pub fn candidates<'a>(
        &self,
        words: &'a [Word],
    ) -> latin_vocab::Result<Vec<(&'a Word, Vec<Table>)>> {
        let mut seen = BTreeSet::new();
        let mut out = vec![];
        for w in words.iter().filter(|w| self.accepts(w)) {
            if !seen.insert((&w.lemma, &w.pos)) {
                continue;
            }
            let tables = self.tables(w)?;
            if !tables.is_empty() {
                out.push((w, tables));
            }
        }
        Ok(out)
    }
}
pub fn random(seed: &mut u64, count: usize) -> usize {
    *seed = seed.wrapping_add(0x9e3779b97f4a7c15);
    let mut z = *seed;
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d049bb133111eb);
    ((z ^ (z >> 31)) % count as u64) as usize
}
