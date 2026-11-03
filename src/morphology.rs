//! Classical Latin verb generation. Rules: Allen & Greenough §§164–190.
//! Keys are mood.voice.tense.person (1s..3p), or form.voice.tense / gerund.case.
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub type Table = Vec<Vec<String>>;
pub type Forms = BTreeMap<String, Vec<String>>;
const PERSONS: [&str; 6] = ["1s", "2s", "3s", "1p", "2p", "3p"];
const TENSES: [&str; 6] = [
    "present",
    "imperfect",
    "future",
    "perfect",
    "pluperfect",
    "future_perfect",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Conjugation {
    First,
    Second,
    Third,
    ThirdIo,
    Fourth,
}
impl Conjugation {
    pub fn label(self) -> &'static str {
        match self {
            Self::First => "第1活用",
            Self::Second => "第2活用",
            Self::Third => "第3活用",
            Self::ThirdIo => "第3活用 -iō 型",
            Self::Fourth => "第4活用",
        }
    }
}
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Passive {
    #[default]
    Full,
    None,
    Impersonal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PrincipalParts {
    pub present: String,
    pub infinitive: String,
    #[serde(default)]
    pub perfect: Vec<String>,
    #[serde(default)]
    pub supine: Vec<String>,
    /// Deponents use a perfect participle instead of a synthetic perfect.
    #[serde(default)]
    pub perfect_participle: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RegularVerb {
    pub principal_parts: PrincipalParts,
    pub conjugation: Conjugation,
    #[serde(default)]
    pub deponent: bool,
    #[serde(default)]
    pub passive: Passive,
    /// dō/dare: short thematic a, except dās and dā.
    #[serde(default)]
    pub short_a: bool,
    /// e.g. morior: mortu- for the perfect participle, morit- for the future.
    #[serde(default)]
    pub future_participle_stem: Option<String>,
    /// Explicit alternatives replace the generated cell. [] suppresses a form.
    #[serde(default)]
    pub overrides: Forms,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Verb {
    Regular(RegularVerb),
    /// Tables live in Word.tables / the paradigms relation, never generated.
    Irregular {
        infinitive: String,
    },
}

pub fn is_stored_irregular(lemma: &str, infinitive: &str) -> bool {
    matches!(
        (lemma, infinitive),
        ("sum", "esse")
            | ("possum", "posse")
            | ("ferō", "ferre")
            | ("eō", "īre")
            | ("volō", "velle")
            | ("nōlō", "nōlle")
            | ("mālō", "mālle")
            | ("fīō", "fierī")
    )
}

#[derive(Debug, PartialEq, Eq)]
pub struct Stems {
    pub present: String,
    pub perfect: Vec<String>,
    pub supine: Vec<String>,
}

impl RegularVerb {
    fn base(&self) -> Result<String, String> {
        use Conjugation::*;
        let ending = match (self.conjugation, self.deponent, self.short_a) {
            (First, false, true) => "are",
            (First, true, true) => "arī",
            (First, false, false) => "āre",
            (First, true, false) => "ārī",
            (Second, false, _) => "ēre",
            (Second, true, _) => "ērī",
            (Third | ThirdIo, false, _) => "ere",
            (Third | ThirdIo, true, _) => "ī",
            (Fourth, false, _) => "īre",
            (Fourth, true, _) => "īrī",
        };
        self.principal_parts
            .infinitive
            .strip_suffix(ending)
            .filter(|s| !s.is_empty())
            .map(str::to_owned)
            .ok_or_else(|| {
                format!(
                    "{} does not match {:?}",
                    self.principal_parts.infinitive, self.conjugation
                )
            })
    }
    pub fn stems(&self) -> Result<Stems, String> {
        let base = self.base()?;
        let present = format!(
            "{base}{}",
            match self.conjugation {
                Conjugation::First =>
                    if self.short_a {
                        "a"
                    } else {
                        "ā"
                    },
                Conjugation::Second => "ē",
                Conjugation::Third => "",
                Conjugation::ThirdIo => "i",
                Conjugation::Fourth => "ī",
            }
        );
        let perfect = self
            .principal_parts
            .perfect
            .iter()
            .map(|p| {
                p.strip_suffix('ī')
                    .filter(|s| !s.is_empty())
                    .map(str::to_owned)
                    .ok_or_else(|| format!("Invalid perfect: {p}"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let mut supine = self
            .principal_parts
            .supine
            .iter()
            .map(|p| {
                p.strip_suffix("um")
                    .filter(|s| !s.is_empty())
                    .map(str::to_owned)
                    .ok_or_else(|| format!("Invalid supine: {p}"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        if let Some(p) = &self.principal_parts.perfect_participle {
            let stem = p
                .strip_suffix("us")
                .filter(|s| !s.is_empty())
                .ok_or_else(|| format!("Invalid perfect participle: {p}"))?;
            if !supine.iter().any(|s| s == stem) {
                supine.push(stem.into());
            }
        }
        Ok(Stems {
            present,
            perfect,
            supine,
        })
    }
    pub fn validate(&self) -> Result<(), String> {
        self.stems()?;
        if self.principal_parts.present.is_empty() {
            return Err("Missing present principal part".into());
        }
        if self.short_a && self.conjugation != Conjugation::First {
            return Err("short_a requires first conjugation".into());
        }
        if is_stored_irregular(
            &self.principal_parts.present,
            &self.principal_parts.infinitive,
        ) {
            return Err("Important irregular verbs require stored tables".into());
        }
        if self.deponent && !self.principal_parts.perfect.is_empty() {
            return Err("A deponent must use a perfect participle".into());
        }
        if self
            .future_participle_stem
            .as_ref()
            .is_some_and(|s| s.is_empty())
        {
            return Err("Empty future participle stem".into());
        }
        let generated = self.generate_base()?;
        for (key, values) in &self.overrides {
            if !generated.contains_key(key) {
                return Err(format!("Unknown form key: {key}"));
            }
            if values.iter().any(|s| s.trim().is_empty()) {
                return Err(format!("Blank override: {key}; use [] for a missing form"));
            }
        }
        Ok(())
    }
    pub fn generate(&self) -> Result<Forms, String> {
        self.validate()?;
        let mut result = self.generate_base()?;
        for (key, values) in &self.overrides {
            result.insert(key.clone(), values.clone());
        }
        Ok(result)
    }
    fn generate_base(&self) -> Result<Forms, String> {
        let base = self.base()?;
        let stems = self.stems()?;
        let mut out = Forms::new();
        for mood in ["indicative", "subjunctive", "imperative"] {
            let tenses: &[&str] = match mood {
                "indicative" => &TENSES,
                "subjunctive" => &["present", "imperfect", "perfect", "pluperfect"],
                _ => &["present", "future"],
            };
            for voice in ["active", "passive"] {
                for tense in tenses {
                    for (person, p) in PERSONS.iter().enumerate() {
                        let key = format!("{mood}.{voice}.{tense}.{p}");
                        let passive_shape = voice == "passive" || self.deponent;
                        let allowed = voice == "active"
                            || (!self.deponent
                                && match self.passive {
                                    Passive::None => false,
                                    Passive::Impersonal => person == 2,
                                    Passive::Full => true,
                                });
                        let mut values = Vec::new();
                        if allowed {
                            if ["perfect", "pluperfect", "future_perfect"].contains(tense) {
                                if passive_shape {
                                    let aux = auxiliary(mood, tense, person);
                                    let endings: &[&str] = if voice == "passive"
                                        && self.passive == Passive::Impersonal
                                    {
                                        &["um"]
                                    } else if person < 3 {
                                        &["us", "a", "um"]
                                    } else {
                                        &["ī", "ae", "a"]
                                    };
                                    for stem in &stems.supine {
                                        for end in endings {
                                            values.push(format!("{stem}{end} {aux}"));
                                        }
                                    }
                                } else {
                                    for stem in &stems.perfect {
                                        values.push(format!(
                                            "{stem}{}",
                                            perfect_suffix(mood, tense, person)
                                        ));
                                    }
                                    if mood == "indicative" && *tense == "perfect" && person == 5 {
                                        for stem in &stems.perfect {
                                            values.push(format!("{stem}ēre"));
                                        }
                                    }
                                }
                            } else if let Some(suffix) =
                                self.present_suffix(mood, tense, passive_shape, person)
                            {
                                let keep_long_a = *tense == "present"
                                    && person == 1
                                    && voice == "active"
                                    && matches!(mood, "indicative" | "imperative");
                                let suffix = if self.short_a && !keep_long_a {
                                    suffix.replacen('ā', "a", 1)
                                } else {
                                    suffix
                                };
                                values.push(
                                    if mood == "indicative"
                                        && *tense == "present"
                                        && person == 0
                                        && voice == "active"
                                    {
                                        self.principal_parts.present.clone()
                                    } else {
                                        format!("{base}{suffix}")
                                    },
                                );
                                if passive_shape
                                    && person == 1
                                    && mood != "imperative"
                                    && let Some(root) = values[0].strip_suffix("ris")
                                {
                                    values.push(format!("{root}re"));
                                }
                            }
                        }
                        out.insert(key, values);
                    }
                }
            }
        }
        self.nonfinite(&base, &stems, &mut out);
        Ok(out)
    }
    fn present_suffix(&self, mood: &str, tense: &str, passive: bool, p: usize) -> Option<String> {
        use Conjugation::*;
        let c = self.conjugation;
        let choose = |a: [&str; 6], b: [&str; 6]| if passive { b[p] } else { a[p] }.to_owned();
        let a = ["am", "ās", "at", "āmus", "ātis", "ant"];
        let ap = ["ar", "āris", "ātur", "āmur", "āminī", "antur"];
        let e = ["em", "ēs", "et", "ēmus", "ētis", "ent"];
        let ep = ["er", "ēris", "ētur", "ēmur", "ēminī", "entur"];
        Some(match (mood, tense) {
            ("indicative", "present") => match c {
                First => choose(
                    ["ō", "ās", "at", "āmus", "ātis", "ant"],
                    ["or", "āris", "ātur", "āmur", "āminī", "antur"],
                ),
                Second => choose(
                    ["eō", "ēs", "et", "ēmus", "ētis", "ent"],
                    ["eor", "ēris", "ētur", "ēmur", "ēminī", "entur"],
                ),
                Third => choose(
                    ["ō", "is", "it", "imus", "itis", "unt"],
                    ["or", "eris", "itur", "imur", "iminī", "untur"],
                ),
                ThirdIo => choose(
                    ["iō", "is", "it", "imus", "itis", "iunt"],
                    ["ior", "eris", "itur", "imur", "iminī", "iuntur"],
                ),
                Fourth => choose(
                    ["iō", "īs", "it", "īmus", "ītis", "iunt"],
                    ["ior", "īris", "ītur", "īmur", "īminī", "iuntur"],
                ),
            },
            ("indicative", "imperfect") => format!(
                "{}{}",
                match c {
                    First => "āb",
                    Second | Third => "ēb",
                    ThirdIo | Fourth => "iēb",
                },
                choose(a, ap)
            ),
            ("indicative", "future") if matches!(c, First | Second) => format!(
                "{}{}",
                if c == First { "āb" } else { "ēb" },
                choose(
                    ["ō", "is", "it", "imus", "itis", "unt"],
                    ["or", "eris", "itur", "imur", "iminī", "untur"]
                )
            ),
            ("indicative", "future") => format!(
                "{}{}",
                if c == Third { "" } else { "i" },
                choose(
                    ["am", "ēs", "et", "ēmus", "ētis", "ent"],
                    ["ar", "ēris", "ētur", "ēmur", "ēminī", "entur"]
                )
            ),
            ("subjunctive", "present") if c == First => choose(e, ep),
            ("subjunctive", "present") => format!(
                "{}{}",
                match c {
                    Second => "e",
                    Third => "",
                    _ => "i",
                },
                choose(a, ap)
            ),
            ("subjunctive", "imperfect") => format!(
                "{}{}",
                match c {
                    First => "ār",
                    Second => "ēr",
                    Third | ThirdIo => "er",
                    Fourth => "īr",
                },
                choose(e, ep)
            ),
            ("imperative", "present") => {
                let theme = match c {
                    First => "ā",
                    Second => "ē",
                    Third | ThirdIo => "e",
                    Fourth => "ī",
                };
                match (passive, p) {
                    (false, 1) => theme.into(),
                    (true, 1) => format!("{theme}re"),
                    (false, 4) => format!(
                        "{}te",
                        if matches!(c, Third | ThirdIo) {
                            "i"
                        } else {
                            theme
                        }
                    ),
                    (true, 4) => format!(
                        "{}minī",
                        if matches!(c, Third | ThirdIo) {
                            "i"
                        } else {
                            theme
                        }
                    ),
                    _ => return None,
                }
            }
            ("imperative", "future") => {
                let theme = match c {
                    First => "ā",
                    Second => "ē",
                    Third | ThirdIo => "i",
                    Fourth => "ī",
                };
                match (passive, p) {
                    (false, 1 | 2) => format!("{theme}tō"),
                    (true, 1 | 2) => format!("{theme}tor"),
                    (false, 4) => format!("{theme}tōte"),
                    (_, 5) => format!(
                        "{}{}",
                        match c {
                            First => "an",
                            Second => "en",
                            Third => "un",
                            ThirdIo | Fourth => "iun",
                        },
                        if passive { "tor" } else { "tō" }
                    ),
                    _ => return None,
                }
            }
            _ => return None,
        })
    }
    fn nonfinite(&self, base: &str, stems: &Stems, out: &mut Forms) {
        use Conjugation::*;
        let suffix = |value: &str| {
            format!(
                "{base}{}",
                if self.short_a {
                    value.replace('ā', "a")
                } else {
                    value.into()
                }
            )
        };
        let active_inf = suffix(match self.conjugation {
            First => "āre",
            Second => "ēre",
            Third | ThirdIo => "ere",
            Fourth => "īre",
        });
        let passive_inf = suffix(match self.conjugation {
            First => "ārī",
            Second => "ērī",
            Third | ThirdIo => "ī",
            Fourth => "īrī",
        });
        let participle = suffix(match self.conjugation {
            First => "āns",
            Second | Third => "ēns",
            ThirdIo | Fourth => "iēns",
        });
        let gerund = suffix(match self.conjugation {
            First => "and",
            Second | Third => "end",
            ThirdIo | Fourth => "iend",
        });
        let future_stems = self
            .future_participle_stem
            .as_ref()
            .map(|s| vec![s.clone()])
            .unwrap_or_else(|| stems.supine.clone());
        let nominal = |roots: &[String], endings: &[&str]| {
            roots
                .iter()
                .flat_map(|s| endings.iter().map(move |e| format!("{s}{e}")))
                .collect::<Vec<_>>()
        };
        let infinitive = |roots: &[String], endings: &[&str], aux: &str| {
            nominal(roots, endings)
                .iter()
                .map(|s| format!("{s} {aux}"))
                .collect::<Vec<_>>()
        };
        let acc = ["um", "am", "ōs", "ās", "a"];
        out.insert(
            "infinitive.active.present".into(),
            vec![if self.deponent {
                self.principal_parts.infinitive.clone()
            } else {
                active_inf
            }],
        );
        out.insert(
            "infinitive.passive.present".into(),
            if !self.deponent && self.passive != Passive::None {
                vec![passive_inf]
            } else {
                vec![]
            },
        );
        out.insert(
            "infinitive.active.perfect".into(),
            if self.deponent {
                infinitive(&stems.supine, &acc, "esse")
            } else {
                nominal(&stems.perfect, &["isse"])
            },
        );
        out.insert(
            "infinitive.passive.perfect".into(),
            if self.deponent || self.passive == Passive::None {
                vec![]
            } else {
                infinitive(
                    &stems.supine,
                    if self.passive == Passive::Impersonal {
                        &["um"]
                    } else {
                        &acc
                    },
                    "esse",
                )
            },
        );
        out.insert(
            "infinitive.active.future".into(),
            infinitive(
                &future_stems,
                &["ūrum", "ūram", "ūrōs", "ūrās", "ūra"],
                "esse",
            ),
        );
        out.insert(
            "infinitive.passive.future".into(),
            if self.deponent || self.passive == Passive::None {
                vec![]
            } else {
                infinitive(&stems.supine, &["um"], "īrī")
            },
        );
        out.insert("participle.active.present".into(), vec![participle]);
        out.insert(
            "participle.active.future".into(),
            nominal(&future_stems, &["ūrus", "ūra", "ūrum"]),
        );
        out.insert(
            "participle.active.perfect".into(),
            if self.deponent {
                nominal(&stems.supine, &["us", "a", "um"])
            } else {
                vec![]
            },
        );
        out.insert(
            "participle.passive.perfect".into(),
            if self.deponent || self.passive == Passive::None {
                vec![]
            } else {
                nominal(
                    &stems.supine,
                    if self.passive == Passive::Impersonal {
                        &["um"]
                    } else {
                        &["us", "a", "um"]
                    },
                )
            },
        );
        out.insert(
            "participle.passive.future".into(),
            if self.passive == Passive::None {
                vec![]
            } else {
                nominal(
                    std::slice::from_ref(&gerund),
                    if self.passive == Passive::Impersonal {
                        &["um"]
                    } else {
                        &["us", "a", "um"]
                    },
                )
            },
        );
        for (case, end) in [
            ("genitive", "ī"),
            ("dative", "ō"),
            ("accusative", "um"),
            ("ablative", "ō"),
        ] {
            out.insert(format!("gerund.{case}"), vec![format!("{gerund}{end}")]);
        }
        out.insert("supine.accusative".into(), nominal(&stems.supine, &["um"]));
        out.insert("supine.ablative".into(), nominal(&stems.supine, &["ū"]));
    }
    pub fn tables(&self) -> Result<Vec<Table>, String> {
        let forms = self.generate()?;
        let mut tables = Vec::new();
        for (mood, mlabel) in [
            ("indicative", "直説法"),
            ("subjunctive", "接続法"),
            ("imperative", "命令法"),
        ] {
            for (voice, vlabel) in [
                (
                    "active",
                    if self.deponent {
                        "異態動詞（能動の意味）"
                    } else {
                        "能動態"
                    },
                ),
                ("passive", "受動態"),
            ] {
                for (tense, label) in TENSES.into_iter().zip([
                    "現在",
                    "未完了過去",
                    "未来",
                    "完了",
                    "過去完了",
                    "未来完了",
                ]) {
                    let cells = PERSONS
                        .iter()
                        .map(|p| forms.get(&format!("{mood}.{voice}.{tense}.{p}")))
                        .collect::<Vec<_>>();
                    if cells.iter().all(|v| v.is_none_or(|f| f.is_empty())) {
                        continue;
                    }
                    let values: Vec<String> = cells
                        .into_iter()
                        .map(|v| {
                            v.filter(|f| !f.is_empty())
                                .map(|f| f.join(" / "))
                                .unwrap_or_else(|| "—".into())
                        })
                        .collect();
                    let mut table = vec![vec![
                        format!("{mlabel} / {vlabel} / {label}"),
                        "単数".into(),
                        "複数".into(),
                    ]];
                    for person in 0..3 {
                        table.push(vec![
                            format!("{}人称", person + 1),
                            values[person].clone(),
                            values[person + 3].clone(),
                        ]);
                    }
                    tables.push(table);
                }
            }
        }
        let mut table = vec![vec![
            "不定詞・分詞・動名詞・目的分詞".into(),
            "語形（分詞は基本形）".into(),
        ]];
        for (key, values) in forms.iter().filter(|(k, v)| {
            !v.is_empty()
                && !k.starts_with("indicative.")
                && !k.starts_with("subjunctive.")
                && !k.starts_with("imperative.")
        }) {
            table.push(vec![
                key.split('.')
                    .map(|s| match s {
                        "infinitive" => "不定詞",
                        "participle" => "分詞",
                        "gerund" => "動名詞",
                        "supine" => "目的分詞",
                        "active" => "能動",
                        "passive" => "受動",
                        "present" => "現在",
                        "perfect" => "完了",
                        "future" => "未来",
                        "genitive" => "属格",
                        "dative" => "与格",
                        "accusative" => "対格",
                        "ablative" => "奪格",
                        _ => s,
                    })
                    .collect::<Vec<_>>()
                    .join("・"),
                values.join(" / "),
            ]);
        }
        tables.push(table);
        Ok(tables)
    }
}

fn perfect_suffix(mood: &str, tense: &str, p: usize) -> &'static str {
    match (mood, tense) {
        ("indicative", "perfect") => ["ī", "istī", "it", "imus", "istis", "ērunt"][p],
        ("indicative", "pluperfect") => ["eram", "erās", "erat", "erāmus", "erātis", "erant"][p],
        ("indicative", "future_perfect") => ["erō", "eris", "erit", "erimus", "eritis", "erint"][p],
        ("subjunctive", "perfect") => ["erim", "erīs", "erit", "erīmus", "erītis", "erint"][p],
        ("subjunctive", "pluperfect") => {
            ["issem", "issēs", "isset", "issēmus", "issētis", "issent"][p]
        }
        _ => unreachable!(),
    }
}
/// Explicit irregular auxiliary forms: sum is never sent to regular generation.
fn auxiliary(mood: &str, tense: &str, p: usize) -> &'static str {
    match (mood, tense) {
        ("indicative", "perfect") => ["sum", "es", "est", "sumus", "estis", "sunt"][p],
        ("indicative", "pluperfect") => ["eram", "erās", "erat", "erāmus", "erātis", "erant"][p],
        ("indicative", "future_perfect") => ["erō", "eris", "erit", "erimus", "eritis", "erunt"][p],
        ("subjunctive", "perfect") => ["sim", "sīs", "sit", "sīmus", "sītis", "sint"][p],
        ("subjunctive", "pluperfect") => {
            ["essem", "essēs", "esset", "essēmus", "essētis", "essent"][p]
        }
        _ => unreachable!(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn verb(
        present: &str,
        infinitive: &str,
        perfect: &str,
        supine: &str,
        conjugation: Conjugation,
    ) -> RegularVerb {
        RegularVerb {
            principal_parts: PrincipalParts {
                present: present.into(),
                infinitive: infinitive.into(),
                perfect: vec![perfect.into()],
                supine: vec![supine.into()],
                perfect_participle: None,
            },
            conjugation,
            deponent: false,
            passive: Passive::Full,
            short_a: false,
            future_participle_stem: None,
            overrides: Forms::new(),
        }
    }
    #[test]
    fn finite_tables_pair_singular_and_plural_for_each_person() {
        let data: serde_json::Value =
            serde_json::from_str(include_str!("../data/swadesh.json")).unwrap();
        let words: Vec<crate::Word> = serde_json::from_value(data["words"].clone()).unwrap();
        let word = words
            .iter()
            .find(|w| w.lemma == "regō")
            .or_else(|| words.iter().find(|w| w.lemma == "dīcō"))
            .unwrap();
        let Some(Verb::Regular(v)) = &word.verb else {
            panic!()
        };
        let forms = v.generate().unwrap();
        let tables = v.tables().unwrap();
        let present = tables
            .iter()
            .find(|t| t[0][0] == "直説法 / 能動態 / 現在")
            .unwrap();
        assert_eq!(present.len(), 4);
        assert_eq!(&present[0][1..], &["単数", "複数"]);
        for (i, row) in present.iter().skip(1).enumerate() {
            assert_eq!(row.len(), 3);
            assert_eq!(
                row[1],
                forms[&format!("indicative.active.present.{}s", i + 1)].join(" / ")
            );
            assert_eq!(
                row[2],
                forms[&format!("indicative.active.present.{}p", i + 1)].join(" / ")
            );
        }
        let imperative = tables
            .iter()
            .find(|t| t[0][0] == "命令法 / 能動態 / 現在")
            .unwrap();
        assert_eq!(imperative[1][1], "—");
        assert_eq!(imperative[3][2], "—");
    }
    #[test]
    fn all_five_conjugations_have_distinct_present_and_future_patterns() {
        use Conjugation::*;
        let cases = [
            (
                verb("amō", "amāre", "amāvī", "amātum", First),
                ["amō", "amās", "amat", "amāmus", "amātis", "amant"],
                "amābor",
                "amēs",
                "amāre",
            ),
            (
                verb("moneō", "monēre", "monuī", "monitum", Second),
                ["moneō", "monēs", "monet", "monēmus", "monētis", "monent"],
                "monēbor",
                "moneās",
                "monēre",
            ),
            (
                verb("regō", "regere", "rēxī", "rēctum", Third),
                ["regō", "regis", "regit", "regimus", "regitis", "regunt"],
                "regar",
                "regās",
                "regere",
            ),
            (
                verb("capiō", "capere", "cēpī", "captum", ThirdIo),
                ["capiō", "capis", "capit", "capimus", "capitis", "capiunt"],
                "capiar",
                "capiās",
                "capere",
            ),
            (
                verb("audiō", "audīre", "audīvī", "audītum", Fourth),
                ["audiō", "audīs", "audit", "audīmus", "audītis", "audiunt"],
                "audiar",
                "audiās",
                "audīre",
            ),
        ];
        for (v, present, future, subjunctive, passive_alternative) in cases {
            let f = v.generate().unwrap();
            for (person, expected) in PERSONS.iter().zip(present) {
                assert_eq!(
                    f[&format!("indicative.active.present.{person}")],
                    vec![expected]
                );
            }
            assert_eq!(f["indicative.passive.future.1s"], vec![future]);
            assert_eq!(f["subjunctive.active.present.2s"], vec![subjunctive]);
            assert!(f["indicative.passive.present.2s"].contains(&passive_alternative.into()));
        }
    }
    #[test]
    fn principal_parts_determine_each_stem_and_compound_agreement() {
        let v = verb("regō", "regere", "rēxī", "rēctum", Conjugation::Third);
        assert_eq!(
            v.stems().unwrap(),
            Stems {
                present: "reg".into(),
                perfect: vec!["rēx".into()],
                supine: vec!["rēct".into()]
            }
        );
        let f = v.generate().unwrap();
        for (key, expected) in [
            ("indicative.active.perfect.2s", "rēxistī"),
            ("indicative.active.pluperfect.1p", "rēxerāmus"),
            ("indicative.active.future_perfect.3p", "rēxerint"),
            ("subjunctive.active.perfect.2p", "rēxerītis"),
            ("subjunctive.active.pluperfect.3s", "rēxisset"),
            ("subjunctive.passive.imperfect.3s", "regerētur"),
            ("imperative.active.future.3p", "reguntō"),
            ("infinitive.active.perfect", "rēxisse"),
            ("infinitive.passive.present", "regī"),
            ("infinitive.passive.future", "rēctum īrī"),
            ("infinitive.active.future", "rēctūram esse"),
            ("participle.active.present", "regēns"),
            ("gerund.genitive", "regendī"),
            ("supine.ablative", "rēctū"),
        ] {
            assert!(f[key].contains(&expected.into()), "{key}: {:?}", f[key]);
        }
        assert_eq!(
            f["indicative.passive.perfect.1s"],
            vec!["rēctus sum", "rēcta sum", "rēctum sum"]
        );
        assert_eq!(
            f["subjunctive.passive.pluperfect.1p"],
            vec!["rēctī essēmus", "rēctae essēmus", "rēcta essēmus"]
        );
        assert!(f["imperative.active.present.1s"].is_empty());
        assert!(!f.contains_key("subjunctive.active.future.1s"));
    }
    #[test]
    fn overrides_replace_suppress_and_validate_without_becoming_a_paradigm() {
        let mut v = verb("dīcō", "dīcere", "dīxī", "dictum", Conjugation::Third);
        assert_eq!(
            v.generate().unwrap()["imperative.active.present.2s"],
            vec!["dīce"]
        );
        v.overrides
            .insert("imperative.active.present.2s".into(), vec!["dīc".into()]);
        v.overrides
            .insert("imperative.passive.future.3p".into(), vec![]);
        let f = v.generate().unwrap();
        assert_eq!(f["imperative.active.present.2s"], vec!["dīc"]);
        assert!(f["imperative.passive.future.3p"].is_empty());
        let json = serde_json::to_string(&v).unwrap();
        assert!(!json.contains("dīcēbāmus"));
        v.overrides.insert(
            "subjunctive.active.future.1s".into(),
            vec!["invalid".into()],
        );
        assert!(v.generate().is_err());
    }
    #[test]
    fn missing_parts_and_voice_restrictions_do_not_invent_forms() {
        let mut v = verb("timeō", "timēre", "timuī", "timētum", Conjugation::Second);
        v.principal_parts.supine.clear();
        let f = v.generate().unwrap();
        assert!(f["indicative.passive.perfect.3s"].is_empty());
        assert!(f["participle.active.future"].is_empty());
        assert_eq!(f["indicative.active.perfect.1s"], vec!["timuī"]);
        v.passive = Passive::None;
        assert!(v.generate().unwrap()["indicative.passive.present.3s"].is_empty());
        v.passive = Passive::Impersonal;
        assert!(v.generate().unwrap()["indicative.passive.present.2s"].is_empty());
        assert_eq!(
            v.generate().unwrap()["indicative.passive.present.3s"],
            vec!["timētur"]
        );
        v.principal_parts.perfect.clear();
        assert!(v.generate().unwrap()["indicative.active.perfect.1s"].is_empty());
    }
    #[test]
    fn deponents_use_passive_endings_but_active_meaning() {
        let mut v = verb("morior", "morī", "unused", "mortuum", Conjugation::ThirdIo);
        v.deponent = true;
        v.principal_parts.perfect.clear();
        v.principal_parts.supine.clear();
        v.principal_parts.perfect_participle = Some("mortuus".into());
        v.future_participle_stem = Some("morit".into());
        let f = v.generate().unwrap();
        assert_eq!(f["indicative.active.present.3p"], vec!["moriuntur"]);
        assert!(f["indicative.active.perfect.1s"].contains(&"mortuus sum".into()));
        assert!(f["infinitive.active.future"].contains(&"moritūrum esse".into()));
        assert!(f["participle.active.perfect"].contains(&"mortuus".into()));
        assert!(f["indicative.passive.present.1s"].is_empty());
    }
    #[test]
    fn short_thematic_a_does_not_shorten_tense_marker() {
        let mut v = verb("dō", "dare", "dedī", "datum", Conjugation::First);
        v.short_a = true;
        let f = v.generate().unwrap();
        for (key, expected) in [
            ("indicative.active.present.2s", "dās"),
            ("indicative.active.present.1p", "damus"),
            ("indicative.active.imperfect.1p", "dabāmus"),
            ("indicative.passive.imperfect.3s", "dabātur"),
            ("imperative.active.present.2s", "dā"),
            ("indicative.active.future.3s", "dabit"),
        ] {
            assert!(f[key].contains(&expected.into()));
        }
    }
    #[test]
    fn stored_irregulars_are_identified_by_lemma_and_infinitive() {
        for (lemma, inf) in [
            ("sum", "esse"),
            ("possum", "posse"),
            ("ferō", "ferre"),
            ("eō", "īre"),
            ("volō", "velle"),
            ("nōlō", "nōlle"),
            ("mālō", "mālle"),
            ("fīō", "fierī"),
        ] {
            assert!(is_stored_irregular(lemma, inf));
        }
        assert!(!is_stored_irregular("volō", "volāre"));
        assert!(!is_stored_irregular("edō", "edere"));
        let mut v = verb("volō", "volāre", "volāvī", "volātum", Conjugation::First);
        assert!(v.validate().is_ok());
        v.principal_parts.infinitive = "velle".into();
        assert!(v.validate().is_err());
    }
}
