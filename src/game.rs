//! Timed typing game. Questions and generated inflections live only in memory.
use crate::{Word, morphology::Verb};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use unicode_normalization::{UnicodeNormalization, char::is_combining_mark};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Direction {
    #[default]
    JapaneseToLatin,
    LatinToJapanese,
    Inflection,
}
impl Direction {
    pub const ALL: [Self; 3] = [
        Self::JapaneseToLatin,
        Self::LatinToJapanese,
        Self::Inflection,
    ];
    pub fn id(self) -> &'static str {
        match self {
            Self::JapaneseToLatin => "ja_la",
            Self::LatinToJapanese => "la_ja",
            Self::Inflection => "inflection",
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::JapaneseToLatin => "日本語 → ラテン語",
            Self::LatinToJapanese => "ラテン語 → 日本語",
            Self::Inflection => "活用チャレンジ",
        }
    }
}
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Difficulty {
    #[default]
    Normal,
    Hard,
    Extreme,
}
impl Difficulty {
    pub const ALL: [Self; 3] = [Self::Normal, Self::Hard, Self::Extreme];
    pub fn id(self) -> &'static str {
        match self {
            Self::Normal => "normal",
            Self::Hard => "hard",
            Self::Extreme => "extreme",
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Normal => "ノーマル",
            Self::Hard => "ハード",
            Self::Extreme => "ゲキムズ",
        }
    }
    pub fn seconds(self, stage: usize) -> f32 {
        (match self {
            Self::Normal => 20.0 - 2.0 * stage as f32,
            Self::Hard => 12.0 - 2.0 * stage as f32,
            Self::Extreme => 8.0 - stage as f32,
        })
        .max(5.0)
    }
}
#[derive(Debug, Clone, Default)]
pub struct Config {
    pub direction: Direction,
    pub difficulty: Difficulty,
    pub strict_macrons: bool,
    pub unlearned_only: bool,
}

impl Config {
    pub fn record_key(&self) -> String {
        format!(
            "v2:{}:{}:{}:{}",
            self.direction.id(),
            self.difficulty.id(),
            self.strict_macrons,
            self.unlearned_only
        )
    }
}

#[derive(Debug, Clone)]
pub struct Question {
    pub word_number: u32,
    pub prompt: String,
    pub clue: String,
    pub answers: Vec<String>,
    pub japanese: bool,
    pub level: usize,
}
impl Question {
    pub fn accepts(&self, answer: &str, strict: bool) -> bool {
        let key = answer_key(answer, self.japanese, strict);
        !key.is_empty()
            && self
                .answers
                .iter()
                .any(|a| answer_key(a, self.japanese, strict) == key)
    }
    pub fn solution(&self) -> String {
        self.answers.join(" / ")
    }
}

pub fn answer_key(text: &str, japanese: bool, strict: bool) -> String {
    let text = text.nfkc().collect::<String>().to_lowercase();
    if japanese {
        text.chars()
            .filter(|c| !c.is_whitespace() && !matches!(c, '〜' | '～' | '~' | '。' | '、'))
            .map(|c| {
                if ('ァ'..='ヶ').contains(&c) {
                    char::from_u32(c as u32 - 0x60).unwrap()
                } else {
                    c
                }
            })
            .collect()
    } else {
        let text = if strict {
            text.nfc().collect()
        } else {
            text.nfd()
                .filter(|c| !is_combining_mark(*c))
                .collect::<String>()
        };
        text.split_whitespace().collect::<Vec<_>>().join(" ")
    }
}
fn japanese_answers(word: &Word, readings: &BTreeMap<String, Vec<String>>) -> Vec<String> {
    let mut values = vec![word.meaning_ja.clone()];
    let mut plain = String::new();
    let mut depth = 0;
    for c in word.meaning_ja.chars() {
        match c {
            '（' | '(' => depth += 1,
            '）' | ')' => depth = (depth - 1).max(0),
            _ if depth == 0 => plain.push(c),
            _ => {}
        }
    }
    values.push(plain.clone());
    values.extend(plain.split(['・', '／', '/']).map(str::to_owned));
    if let Some(extra) = readings.get(&word.meaning_ja) {
        values.extend(extra.clone());
    }
    values.retain(|s| !answer_key(s, true, false).is_empty());
    values.sort();
    values.dedup();
    values
}
fn latin_forms(cell: &str) -> Vec<String> {
    // Noun cells have comma/space-separated alternatives; reject annotations.
    cell.split(|c: char| c.is_whitespace() || matches!(c, ',' | '/' | '、'))
        .filter(|s| {
            !s.is_empty()
                && s.chars().all(|c| {
                    c.is_ascii_alphabetic() || "āēīōūȳĀĒĪŌŪȲ".contains(c) || is_combining_mark(c)
                })
        })
        .map(str::to_owned)
        .collect()
}

pub fn question_pool(words: &[Word], config: &Config) -> crate::Result<Vec<Question>> {
    let readings: BTreeMap<String, Vec<String>> =
        serde_json::from_str(include_str!("../data/game_readings.json"))?;
    let mut questions = Vec::new();
    for w in words
        .iter()
        .filter(|w| !config.unlearned_only || !w.learned)
    {
        let level = match w.source_form.chars().count() {
            0..=4 => 0,
            5..=6 => 1,
            7..=8 => 2,
            _ => 3,
        };
        let mut add =
            |prompt: String, clue: String, answers: Vec<String>, japanese: bool, level: usize| {
                if !answers.is_empty() {
                    questions.push(Question {
                        word_number: w.number,
                        prompt,
                        clue,
                        answers,
                        japanese,
                        level,
                    });
                }
            };
        match config.direction {
            Direction::JapaneseToLatin => add(
                w.meaning_ja.clone(),
                "ラテン語で答えよう".into(),
                vec![w.source_form.clone()],
                false,
                level,
            ),
            Direction::LatinToJapanese => add(
                w.source_form.clone(),
                "日本語の意味を答えよう".into(),
                japanese_answers(w, &readings),
                true,
                level,
            ),
            Direction::Inflection => {
                if w.pos == "Noun" {
                    for table in &w.tables {
                        if table
                            .first()
                            .is_none_or(|r| r.len() != 3 || r[1] != "singular" || r[2] != "plural")
                        {
                            continue;
                        }
                        for row in table.iter().skip(1).filter(|r| r.len() == 3) {
                            let Some((case, rank)) = (match row[0].as_str() {
                                "nominative" => Some(("主格", 0)),
                                "genitive" => Some(("属格", 1)),
                                "dative" => Some(("与格", 2)),
                                "accusative" => Some(("対格", 1)),
                                "ablative" => Some(("奪格", 2)),
                                "vocative" => Some(("呼格", 2)),
                                _ => None,
                            }) else {
                                continue;
                            };
                            for (col, number) in [(1, "単数"), (2, "複数")] {
                                add(
                                    w.lemma.clone(),
                                    format!("{}  /  {number}・{case}", w.meaning_ja),
                                    latin_forms(&row[col]),
                                    false,
                                    (rank + col - 1).min(3),
                                );
                            }
                        }
                    }
                }
                if let Some(Verb::Regular(v)) = &w.verb {
                    for (key, forms) in v.generate()? {
                        let parts = key.split('.').collect::<Vec<_>>();
                        if parts.len() != 4 || forms.is_empty() {
                            continue;
                        }
                        let (mood, rank) = match parts[0] {
                            "indicative" => ("直説法", 0),
                            "subjunctive" => ("接続法", 2),
                            "imperative" => ("命令法", 1),
                            _ => continue,
                        };
                        let (tense, trank) = match parts[2] {
                            "present" => ("現在", 0),
                            "imperfect" => ("未完了過去", 1),
                            "future" => ("未来", 1),
                            "perfect" => ("完了", 1),
                            "pluperfect" => ("過去完了", 2),
                            "future_perfect" => ("未来完了", 2),
                            _ => continue,
                        };
                        let voice = if v.deponent {
                            "異態"
                        } else if parts[1] == "active" {
                            "能動"
                        } else {
                            "受動"
                        };
                        let person = match parts[3] {
                            "1s" => "単数1人称",
                            "2s" => "単数2人称",
                            "3s" => "単数3人称",
                            "1p" => "複数1人称",
                            "2p" => "複数2人称",
                            "3p" => "複数3人称",
                            _ => continue,
                        };
                        add(
                            w.lemma.clone(),
                            format!("{mood}・{voice}・{tense}・{person}"),
                            forms,
                            false,
                            (rank + trank).min(3),
                        );
                    }
                }
            }
        }
    }
    // Identical prompts can have several valid meanings (e.g. longus, cum).
    let mut merged: BTreeMap<(String, String), Question> = BTreeMap::new();
    for q in questions {
        let key = (q.prompt.nfc().collect(), q.clue.clone());
        merged
            .entry(key)
            .and_modify(|existing| {
                existing.answers.extend(q.answers.clone());
                existing.answers.sort();
                existing.answers.dedup();
            })
            .or_insert(q);
    }
    Ok(merged.into_values().collect())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Correct,
    Wrong,
    Timeout,
    Skipped,
}
#[derive(Debug, Clone)]
pub struct Attempt {
    pub original_index: usize,
    pub is_review: bool,
    pub life_recovered: bool,
    pub question: Question,
    pub answer: String,
    pub outcome: Outcome,
    pub points: u32,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    Playing,
    Feedback,
    Finished,
}
#[derive(Debug)]
pub struct Run {
    normal_cursor: usize,
    review: Option<usize>,
    review_queue: std::collections::VecDeque<(usize, usize)>,
    pub config: Config,
    pub questions: Vec<Question>,
    pub attempts: Vec<Attempt>,
    pub phase: Phase,
    pub score: u32,
    pub combo: u32,
    pub best_combo: u32,
    pub lives: u32,
    pub remaining: f32,
    pub paused: bool,
}
impl Run {
    pub fn start(pool: Vec<Question>, config: Config, mut seed: u64) -> Option<Self> {
        // SplitMix64 + Fisher–Yates: reproducible tests, varied offline play.
        let mut pool = pool;
        for i in (1..pool.len()).rev() {
            seed = seed.wrapping_add(0x9e3779b97f4a7c15);
            let mut z = seed;
            z = (z ^ (z >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94d049bb133111eb);
            z ^= z >> 31;
            pool.swap(i, (z % (i as u64 + 1)) as usize);
        }
        let mut questions = Vec::new();
        let mut used = BTreeSet::new();
        for index in 0..16 {
            let desired = index / 4;
            let candidate = pool
                .iter()
                .enumerate()
                .filter(|(_, q)| !used.contains(&q.word_number))
                .min_by_key(|(_, q)| q.level.abs_diff(desired))
                .map(|(i, _)| i);
            let Some(i) = candidate else {
                break;
            };
            let q = pool.remove(i);
            used.insert(q.word_number);
            questions.push(q);
        }
        if questions.is_empty() {
            return None;
        }
        let remaining = config.difficulty.seconds(0);
        Some(Self {
            normal_cursor: 0,
            review: None,
            review_queue: Default::default(),
            config,
            questions,
            attempts: vec![],
            phase: Phase::Playing,
            score: 0,
            combo: 0,
            best_combo: 0,
            lives: 3,
            remaining,
            paused: false,
        })
    }
    pub fn current_index(&self) -> usize {
        self.review.unwrap_or(self.normal_cursor)
    }
    pub fn is_review(&self) -> bool {
        self.review.is_some()
    }
    pub fn current(&self) -> Option<&Question> {
        self.questions.get(self.current_index())
    }
    pub fn stage(&self) -> usize {
        self.current_index()
            .min(self.questions.len().saturating_sub(1))
            / 4
    }
    pub fn limit(&self) -> f32 {
        self.config.difficulty.seconds(self.stage())
    }
    pub fn cleared(&self) -> bool {
        self.phase == Phase::Finished && self.lives > 0
    }
    pub fn tick(&mut self, seconds: f32) {
        if self.phase != Phase::Playing || self.paused || !seconds.is_finite() || seconds < 0.0 {
            return;
        }
        self.remaining = (self.remaining - seconds).max(0.0);
        if self.remaining == 0.0 {
            self.resolve(String::new(), Outcome::Timeout);
        }
    }
    pub fn submit(&mut self, answer: &str) {
        if self.phase != Phase::Playing || self.paused || answer.trim().is_empty() {
            return;
        }
        let correct = self
            .current()
            .is_some_and(|q| q.accepts(answer, self.config.strict_macrons));
        self.resolve(
            answer.into(),
            if correct {
                Outcome::Correct
            } else {
                Outcome::Wrong
            },
        );
    }
    pub fn skip(&mut self) {
        if self.phase == Phase::Playing && !self.paused {
            self.resolve(String::new(), Outcome::Skipped);
        }
    }
    fn resolve(&mut self, answer: String, outcome: Outcome) {
        let Some(question) = self.current().cloned() else {
            return;
        };
        let original_index = self.current_index();
        let is_review = self.is_review();
        let life_recovered = is_review && outcome == Outcome::Correct && self.lives < 3;
        if life_recovered {
            self.lives += 1;
        }
        let points = if outcome == Outcome::Correct && is_review {
            0 // Recovery only: intentionally missing cannot farm points or combos.
        } else if outcome == Outcome::Correct {
            self.combo += 1;
            self.best_combo = self.best_combo.max(self.combo);
            100 + (50.0 * self.remaining / self.limit()).round() as u32 + 10 * self.combo.min(10)
        } else {
            self.combo = 0;
            self.lives = self.lives.saturating_sub(1);
            0
        };
        if !is_review && matches!(outcome, Outcome::Wrong | Outcome::Timeout) {
            // Two further normal questions before this one returns. At the end,
            // drain outstanding reviews without inventing filler questions.
            self.review_queue
                .push_back((original_index, self.normal_cursor + 3));
        }
        self.score += points;
        self.attempts.push(Attempt {
            original_index,
            is_review,
            life_recovered,
            question,
            answer,
            outcome,
            points,
        });
        self.phase = Phase::Feedback;
    }
    pub fn advance(&mut self) {
        if self.phase != Phase::Feedback || self.paused {
            return;
        }
        if self.review.take().is_none() {
            self.normal_cursor += 1;
        }
        if self.lives == 0 {
            self.phase = Phase::Finished;
            return;
        }
        if self.review_queue.front().is_some_and(|(_, due)| {
            *due <= self.normal_cursor || self.normal_cursor >= self.questions.len()
        }) {
            self.review = self.review_queue.pop_front().map(|(index, _)| index);
        }
        if self.review.is_none() && self.normal_cursor >= self.questions.len() {
            self.phase = Phase::Finished;
        } else {
            self.phase = Phase::Playing;
            self.remaining = self.limit();
        }
    }
}
