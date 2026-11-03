mod classical;
pub mod game;
pub mod morphology;
use morphology::{Verb, is_stored_irregular};
use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};
use std::path::Path;
use unicode_normalization::{UnicodeNormalization, char::is_combining_mark};

pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Word {
    pub number: u32,
    pub lemma: String,
    pub source_form: String,
    pub meaning_ja: String,
    pub concept_en: String,
    pub source: String,
    pub principal_parts: String,
    pub pos: String,
    pub morphology_status: String,
    pub tables: Vec<Vec<Vec<String>>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verb: Option<Verb>,
    #[serde(default)]
    pub learned: bool,
}

#[derive(Deserialize)]
struct Seed {
    words: Vec<Word>,
}

pub fn normalize(text: &str) -> String {
    text.nfd()
        .filter(|c| !is_combining_mark(*c))
        .collect::<String>()
        .to_lowercase()
}

impl Word {
    pub fn validate(&self) -> Result<()> {
        match (&self.verb, self.pos.as_str()) {
            (Some(Verb::Regular(v)), "Verb") => {
                v.validate()?;
                if v.principal_parts.present != self.lemma {
                    return Err("Verb principal part differs from lemma".into());
                }
                if !self.tables.is_empty() {
                    return Err("Regular verbs must not store paradigms".into());
                }
            }
            (Some(Verb::Irregular { infinitive }), "Verb") => {
                if !is_stored_irregular(&self.lemma, infinitive) || self.tables.is_empty() {
                    return Err(
                        "Stored irregular verb requires an approved lemma/infinitive and paradigms"
                            .into(),
                    );
                }
            }
            (None, "Verb") => {
                return Err(format!("Missing verb specification: {}", self.lemma).into());
            }
            (Some(_), _) => return Err("Only verbs can have a verb specification".into()),
            (None, _) => {}
        }
        Ok(())
    }

    pub fn display_tables(&self) -> Result<Vec<morphology::Table>> {
        match &self.verb {
            Some(Verb::Regular(v)) => Ok(v.tables()?),
            _ => Ok(self.tables.clone()),
        }
    }

    pub fn matches(&self, query: &str) -> bool {
        let query = normalize(query);
        [
            &self.lemma,
            &self.source_form,
            &self.meaning_ja,
            &self.concept_en,
        ]
        .iter()
        .any(|s| normalize(s).contains(&query))
    }
}

pub struct Database {
    connection: Connection,
}

impl Database {
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
            std::fs::create_dir_all(parent)?;
        }
        Self::initialize(Connection::open(path)?)
    }

    fn initialize(mut connection: Connection) -> Result<Self> {
        connection.execute_batch(
            "PRAGMA foreign_keys=ON;
            CREATE TABLE IF NOT EXISTS metadata(key TEXT PRIMARY KEY, value TEXT NOT NULL);
            CREATE TABLE IF NOT EXISTS lexemes(
                id INTEGER PRIMARY KEY, lemma TEXT NOT NULL, pos TEXT NOT NULL,
                principal_parts TEXT NOT NULL, morphology_status TEXT NOT NULL,
                source TEXT NOT NULL, UNIQUE(lemma, pos));
            CREATE TABLE IF NOT EXISTS concepts(
                number INTEGER PRIMARY KEY, lexeme_id INTEGER NOT NULL REFERENCES lexemes(id),
                source_form TEXT NOT NULL, meaning_ja TEXT NOT NULL, concept_en TEXT NOT NULL);
            CREATE TABLE IF NOT EXISTS paradigms(
                lexeme_id INTEGER NOT NULL REFERENCES lexemes(id), ordinal INTEGER NOT NULL,
                cells_json TEXT NOT NULL, PRIMARY KEY(lexeme_id, ordinal));
            CREATE TABLE IF NOT EXISTS progress(
                concept_number INTEGER PRIMARY KEY REFERENCES concepts(number),
                learned INTEGER NOT NULL DEFAULT 0 CHECK(learned IN (0,1)));
            CREATE TABLE IF NOT EXISTS game_records(
                rules_key TEXT PRIMARY KEY,
                best_score INTEGER NOT NULL CHECK(best_score>=0),
                plays INTEGER NOT NULL CHECK(plays>=0));
            CREATE TABLE IF NOT EXISTS verb_specs(
                lexeme_id INTEGER PRIMARY KEY REFERENCES lexemes(id),
                spec_json TEXT NOT NULL CHECK(json_valid(spec_json)));",
        )?;
        let seeded: bool = connection.query_row(
            "SELECT EXISTS(SELECT 1 FROM metadata WHERE key='seed_version')",
            [],
            |r| r.get(0),
        )?;
        if !seeded {
            let seed: Seed = serde_json::from_str(include_str!("../data/swadesh.json"))?;
            let tx = connection.transaction()?;
            for word in seed.words {
                word.validate()?;
                tx.execute("INSERT OR IGNORE INTO lexemes(lemma,pos,principal_parts,morphology_status,source) VALUES(?1,?2,?3,?4,?5)",
                    params![word.lemma,word.pos,word.principal_parts,word.morphology_status,word.source])?;
                let id: i64 = tx.query_row(
                    "SELECT id FROM lexemes WHERE lemma=?1 AND pos=?2",
                    params![word.lemma, word.pos],
                    |r| r.get(0),
                )?;
                tx.execute(
                    "INSERT INTO concepts VALUES(?1,?2,?3,?4,?5)",
                    params![
                        word.number,
                        id,
                        word.source_form,
                        word.meaning_ja,
                        word.concept_en
                    ],
                )?;
                if let Some(verb) = &word.verb {
                    tx.execute(
                        "INSERT OR IGNORE INTO verb_specs VALUES(?1,?2)",
                        params![id, serde_json::to_string(verb)?],
                    )?;
                }
                for (ordinal, table) in word.tables.iter().enumerate() {
                    tx.execute(
                        "INSERT OR IGNORE INTO paradigms VALUES(?1,?2,?3)",
                        params![id, ordinal as i64, serde_json::to_string(table)?],
                    )?;
                }
            }
            tx.execute("INSERT INTO metadata VALUES('seed_version','2')", [])?;
            tx.execute("INSERT INTO metadata VALUES('schema_version','2')", [])?;
            tx.commit()?;
        }
        migrate_verbs(&mut connection)?;
        classical::migrate(&mut connection)?;
        // These guards also protect against accidentally persisting generated UI tables.
        connection.execute_batch("CREATE TRIGGER IF NOT EXISTS regular_paradigm_insert
            BEFORE INSERT ON paradigms WHEN EXISTS(SELECT 1 FROM verb_specs WHERE lexeme_id=NEW.lexeme_id AND json_extract(spec_json,'$.kind')='regular')
            BEGIN SELECT RAISE(ABORT,'regular verbs cannot store paradigms'); END;
            CREATE TRIGGER IF NOT EXISTS regular_paradigm_update
            BEFORE UPDATE ON paradigms WHEN EXISTS(SELECT 1 FROM verb_specs WHERE lexeme_id=NEW.lexeme_id AND json_extract(spec_json,'$.kind')='regular')
            BEGIN SELECT RAISE(ABORT,'regular verbs cannot store paradigms'); END;
            CREATE TRIGGER IF NOT EXISTS regular_spec_insert BEFORE INSERT ON verb_specs
            WHEN json_extract(NEW.spec_json,'$.kind')='regular' AND EXISTS(SELECT 1 FROM paradigms WHERE lexeme_id=NEW.lexeme_id)
            BEGIN SELECT RAISE(ABORT,'remove stored paradigms before setting regular spec'); END;
            CREATE TRIGGER IF NOT EXISTS regular_spec_update BEFORE UPDATE ON verb_specs
            WHEN json_extract(NEW.spec_json,'$.kind')='regular' AND EXISTS(SELECT 1 FROM paradigms WHERE lexeme_id=NEW.lexeme_id)
            BEGIN SELECT RAISE(ABORT,'remove stored paradigms before setting regular spec'); END;")?;
        Ok(Self { connection })
    }

    pub fn words(&self) -> Result<Vec<Word>> {
        let mut stmt = self.connection.prepare("SELECT c.number,l.lemma,c.source_form,c.meaning_ja,c.concept_en,l.source,l.principal_parts,l.pos,l.morphology_status,COALESCE(p.learned,0),l.id,v.spec_json
            FROM concepts c JOIN lexemes l ON l.id=c.lexeme_id LEFT JOIN progress p ON p.concept_number=c.number LEFT JOIN verb_specs v ON v.lexeme_id=l.id ORDER BY c.number")?;
        let mut rows = stmt.query([])?;
        let mut words = Vec::new();
        while let Some(row) = rows.next()? {
            let id: i64 = row.get(10)?;
            let mut tables_stmt = self
                .connection
                .prepare("SELECT cells_json FROM paradigms WHERE lexeme_id=?1 ORDER BY ordinal")?;
            let jsons = tables_stmt
                .query_map([id], |r| r.get::<_, String>(0))?
                .collect::<std::result::Result<Vec<_>, _>>()?;
            let tables = jsons
                .iter()
                .map(|s| serde_json::from_str(s))
                .collect::<std::result::Result<_, _>>()?;
            let spec: Option<String> = row.get(11)?;
            let word = Word {
                number: row.get(0)?,
                lemma: row.get(1)?,
                source_form: row.get(2)?,
                meaning_ja: row.get(3)?,
                concept_en: row.get(4)?,
                source: row.get(5)?,
                principal_parts: row.get(6)?,
                pos: row.get(7)?,
                morphology_status: row.get(8)?,
                learned: row.get(9)?,
                tables,
                verb: spec.map(|json| serde_json::from_str(&json)).transpose()?,
            };
            word.validate()?;
            words.push(word);
        }
        Ok(words)
    }

    pub fn game_best(&self, config: &game::Config) -> Result<u32> {
        use rusqlite::OptionalExtension;
        Ok(self
            .connection
            .query_row(
                "SELECT best_score FROM game_records WHERE rules_key=?1",
                [config.record_key()],
                |r| r.get(0),
            )
            .optional()?
            .unwrap_or(0))
    }
    pub fn record_game(&self, run: &game::Run) -> Result<u32> {
        if run.phase != game::Phase::Finished {
            return Err("Only finished games can be recorded".into());
        }
        self.connection.execute("INSERT INTO game_records VALUES(?1,?2,1)
            ON CONFLICT(rules_key) DO UPDATE SET best_score=MAX(best_score,excluded.best_score),plays=plays+1",params![run.config.record_key(),run.score])?;
        self.game_best(&run.config)
    }

    pub fn set_learned(&self, number: u32, learned: bool) -> Result<()> {
        self.connection.execute("INSERT INTO progress VALUES(?1,?2) ON CONFLICT(concept_number) DO UPDATE SET learned=excluded.learned", params![number,learned])?;
        Ok(())
    }
}

fn migrate_verbs(connection: &mut Connection) -> Result<()> {
    use rusqlite::OptionalExtension;
    let version: Option<String> = connection
        .query_row(
            "SELECT value FROM metadata WHERE key='schema_version'",
            [],
            |r| r.get(0),
        )
        .optional()?;
    match version.as_deref() {
        Some("2") => return Ok(()),
        Some(other) => return Err(format!("Unsupported database schema: {other}").into()),
        None => {}
    }
    let seed: Seed = serde_json::from_str(include_str!("../data/swadesh.json"))?;
    let classical_rules = classical::rules()?;
    let existing = connection
        .prepare("SELECT id,lemma,principal_parts FROM lexemes WHERE pos='Verb'")?
        .query_map([], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
            ))
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    let tx = connection.transaction()?;
    for (id, lemma, parts) in existing {
        let word = seed.words.iter().find(|w| w.pos=="Verb" && w.lemma==lemma && (w.principal_parts==parts || classical_rules.iter().any(|r| r.lemma==lemma && r.pos=="Verb" && r.previous_principal_parts==parts)))
            .ok_or_else(|| format!("Cannot safely migrate edited or unknown verb {lemma}; dictionary metadata must be converted explicitly"))?;
        word.validate()?;
        let verb = word
            .verb
            .as_ref()
            .ok_or("Missing migration specification")?;
        if matches!(verb, Verb::Regular(_)) {
            tx.execute("DELETE FROM paradigms WHERE lexeme_id=?1", [id])?;
        }
        tx.execute(
            "INSERT INTO verb_specs VALUES(?1,?2)",
            params![id, serde_json::to_string(verb)?],
        )?;
        tx.execute(
            "UPDATE lexemes SET morphology_status=?1 WHERE id=?2",
            params![word.morphology_status, id],
        )?;
    }
    tx.execute("INSERT INTO metadata VALUES('schema_version','2')", [])?;
    tx.commit()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn seed_and_morphology_integrity() -> Result<()> {
        let db = Database::initialize(Connection::open_in_memory()?)?;
        let words = db.words()?;
        assert_eq!(words.len(), 207);
        assert_eq!(
            words
                .iter()
                .filter(|w| matches!(w.verb, Some(Verb::Regular(_))))
                .count(),
            56
        );
        for (number, key, expected) in [
            (105, "indicative.passive.present.3s", "olfit"),
            (93, "indicative.active.present.2s", "ēs"),
            (125, "participle.active.future", "stātūrus"),
            (128, "participle.active.present", "dāns"),
            (140, "imperative.active.present.2s", "dīc"),
        ] {
            let word = &words[number - 1];
            let Some(Verb::Regular(v)) = &word.verb else {
                panic!("Expected compact verb")
            };
            assert!(word.tables.is_empty());
            assert!(
                v.generate()?[key].contains(&expected.into()),
                "{} / {key}",
                word.lemma
            );
        }

        assert!(words.iter().all(|w| w.morphology_status != "missing"));
        assert!(words[119].principal_parts.contains("volāre"));
        assert_eq!(words[13].pos, "Conjunction");
        assert_eq!(words[202].pos, "Preposition");
        assert_eq!(words[11].lemma, "quis");
        assert_eq!(words[53].pos, "Noun");
        assert!(words[53].principal_parts.contains("frūctūs"));
        assert!(words[28].principal_parts.contains("lātior"));
        assert!(words.iter().all(|w| !w.pos.is_empty()));
        assert_eq!(
            words.iter().map(|w| w.number).collect::<Vec<_>>(),
            (1..=207).collect::<Vec<_>>()
        );
        for word in &words {
            assert!(!word.lemma.is_empty() && !word.meaning_ja.is_empty());
            if word.morphology_status == "imported" {
                assert!(!word.tables.is_empty());
            }
            for table in &word.tables {
                assert!(table.iter().all(|row| row.len() == table[0].len()));
            }
        }
        let aqua = words.iter().find(|w| w.lemma == "aqua").unwrap();
        assert!(
            aqua.tables
                .iter()
                .flatten()
                .flatten()
                .any(|s| s == "aquārum")
        );
        let bone = &words[64];
        let mouth = &words[75];
        assert_ne!(bone.lemma, mouth.lemma);
        assert!(bone.tables.iter().flatten().flatten().any(|s| s == "ossis"));
        assert!(mouth.tables.iter().flatten().flatten().any(|s| s == "ōris"));
        assert!(words[147].matches("LUNA"));
        assert!(words[147].matches("月"));
        assert!(!words[147].matches("missing"));
        Ok(())
    }
    #[test]
    fn progress_survives_restart_and_seeding() -> Result<()> {
        let path =
            std::env::temp_dir().join(format!("latin-vocab-test-{}.sqlite3", std::process::id()));
        {
            let db = Database::open(&path)?;
            db.set_learned(150, true)?;
            assert!(db.set_learned(999, true).is_err());
        }
        let db = Database::open(&path)?;
        assert_eq!(db.words()?.len(), 207);
        assert!(db.words()?[149].learned);
        db.set_learned(150, false)?;
        assert!(!db.words()?[149].learned);
        drop(db);
        std::fs::remove_file(path)?;
        Ok(())
    }
    // Simulate the v1 format without keeping full regular verb tables in the repo.
    fn downgrade_to_v1(db: &Database) -> Result<()> {
        db.connection.execute_batch("DROP TRIGGER regular_paradigm_insert;
            DROP TRIGGER regular_paradigm_update;
            DROP TRIGGER regular_spec_insert;
            DROP TRIGGER regular_spec_update;
            DELETE FROM verb_specs;
            DELETE FROM metadata WHERE key='schema_version';
            UPDATE metadata SET value='1' WHERE key='seed_version';
            UPDATE lexemes SET morphology_status='imported' WHERE pos='Verb';
            INSERT INTO paradigms SELECT id,0,'[[\"legacy verb table\"]]' FROM lexemes WHERE pos='Verb';")?;
        Ok(())
    }
    #[test]
    fn migration_removes_only_regular_tables_and_preserves_learning_and_nouns() -> Result<()> {
        let mut db = Database::initialize(Connection::open_in_memory()?)?;
        db.set_learned(92, true)?;
        db.set_learned(150, true)?;
        let before = db.words()?;
        downgrade_to_v1(&db)?;
        migrate_verbs(&mut db.connection)?;
        // A second migration must be a no-op.
        migrate_verbs(&mut db.connection)?;
        let after = db.words()?;
        assert_eq!(after.len(), before.len());
        for (old, new) in before.iter().zip(&after) {
            assert_eq!(old.number, new.number);
            assert_eq!(old.meaning_ja, new.meaning_ja);
            assert_eq!(old.learned, new.learned);
            assert_eq!(old.source, new.source);
            if old.pos != "Verb" {
                assert_eq!(old.tables, new.tables);
            } else {
                assert!(new.tables.is_empty());
                assert!(!new.display_tables()?.is_empty());
                assert!(matches!(new.verb, Some(Verb::Regular(_))));
            }
        }
        assert_eq!(db.connection.query_row("SELECT COUNT(*) FROM paradigms p JOIN lexemes l ON l.id=p.lexeme_id WHERE l.pos='Verb'",[],|r| r.get::<_,i64>(0))?,0);
        Ok(())
    }
    #[test]
    fn failed_migration_rolls_back_instead_of_discarding_edited_data() -> Result<()> {
        let mut db = Database::initialize(Connection::open_in_memory()?)?;
        downgrade_to_v1(&db)?;
        db.connection.execute(
            "UPDATE lexemes SET principal_parts='custom user data' WHERE lemma='ūrō'",
            [],
        )?;
        let count = db
            .connection
            .query_row("SELECT COUNT(*) FROM paradigms", [], |r| r.get::<_, i64>(0))?;
        let error = migrate_verbs(&mut db.connection).unwrap_err();
        assert!(error.to_string().contains("ūrō"));
        assert_eq!(
            db.connection
                .query_row("SELECT COUNT(*) FROM paradigms", [], |r| r.get::<_, i64>(0))?,
            count
        );
        assert_eq!(
            db.connection
                .query_row("SELECT COUNT(*) FROM verb_specs", [], |r| r
                    .get::<_, i64>(0))?,
            0
        );
        assert_eq!(
            db.connection.query_row(
                "SELECT COUNT(*) FROM metadata WHERE key='schema_version'",
                [],
                |r| r.get::<_, i64>(0)
            )?,
            0
        );
        Ok(())
    }
    #[test]
    fn storage_guards_reject_regular_tables_and_allow_explicit_irregulars() -> Result<()> {
        let db = Database::initialize(Connection::open_in_memory()?)?;
        let regular_id =
            db.connection
                .query_row("SELECT id FROM lexemes WHERE lemma='bibō'", [], |r| {
                    r.get::<_, i64>(0)
                })?;
        assert!(
            db.connection
                .execute(
                    "INSERT INTO paradigms VALUES(?1,0,'[[\"bibō\"]]')",
                    [regular_id]
                )
                .is_err()
        );
        let mut sum = db.words()?[91].clone();
        sum.number = 208;
        sum.lemma = "sum".into();
        sum.source_form = "sum".into();
        sum.meaning_ja = "ある・いる".into();
        sum.principal_parts = "sum, esse, fuī".into();
        sum.verb = Some(Verb::Irregular {
            infinitive: "esse".into(),
        });
        sum.morphology_status = "imported".into();
        // Representative supplied table: persistence must be lossless, with no generation.
        sum.tables = vec![vec![vec![
            "sum".into(),
            "es".into(),
            "est".into(),
            "sumus".into(),
            "estis".into(),
            "sunt".into(),
        ]]];
        sum.validate()?;
        db.connection.execute("INSERT INTO lexemes(lemma,pos,principal_parts,morphology_status,source) VALUES('sum','Verb',?1,'imported',?2)",params![sum.principal_parts,sum.source])?;
        let id = db.connection.last_insert_rowid();
        db.connection.execute(
            "INSERT INTO concepts VALUES(208,?1,'sum','ある・いる','be')",
            [id],
        )?;
        db.connection.execute(
            "INSERT INTO verb_specs VALUES(?1,?2)",
            params![id, serde_json::to_string(sum.verb.as_ref().unwrap())?],
        )?;
        db.connection.execute(
            "INSERT INTO paradigms VALUES(?1,0,?2)",
            params![id, serde_json::to_string(&sum.tables[0])?],
        )?;
        let reread = db.words()?.pop().unwrap();
        assert_eq!(reread.tables, sum.tables);
        assert_eq!(reread.display_tables()?, sum.tables);
        assert!(matches!(reread.verb, Some(Verb::Irregular { .. })));
        Ok(())
    }
}
