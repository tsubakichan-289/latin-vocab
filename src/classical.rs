//! Explicit classical teaching edition. Corrections are shared with the importer.
use crate::{
    Result,
    morphology::{Table, Verb},
};
use rusqlite::{Connection, OptionalExtension, params};
use serde::Deserialize;
#[derive(Deserialize)]
pub(crate) struct Rule {
    pub lemma: String,
    pub pos: String,
    pub previous_tables: Vec<Table>,
    pub tables: Vec<Table>,
    pub previous_verb: Option<Verb>,
    pub verb: Option<Verb>,
    pub previous_principal_parts: String,
    pub principal_parts: String,
}
#[derive(Deserialize)]
struct Profile {
    edition: String,
    rules: Vec<Rule>,
}
pub(crate) fn rules() -> Result<Vec<Rule>> {
    Ok(serde_json::from_str::<Profile>(include_str!("../data/classical_profile.json"))?.rules)
}
pub(crate) fn migrate(connection: &mut Connection) -> Result<()> {
    let profile: Profile = serde_json::from_str(include_str!("../data/classical_profile.json"))?;
    let edition: Option<String> = connection
        .query_row(
            "SELECT value FROM metadata WHERE key='morphology_edition'",
            [],
            |r| r.get(0),
        )
        .optional()?;
    if edition.as_deref() == Some(&profile.edition) {
        return Ok(());
    }
    let tx = connection.transaction()?;
    for rule in profile.rules {
        let row: Option<(i64, String)> = tx
            .query_row(
                "SELECT id,principal_parts FROM lexemes WHERE lemma=?1 AND pos=?2",
                params![rule.lemma, rule.pos],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        let Some((id, parts)) = row else {
            continue;
        };
        let tables = tx
            .prepare("SELECT cells_json FROM paradigms WHERE lexeme_id=?1 ORDER BY ordinal")?
            .query_map([id], |r| r.get::<_, String>(0))?
            .collect::<std::result::Result<Vec<_>, _>>()?
            .into_iter()
            .map(|s| serde_json::from_str::<Table>(&s))
            .collect::<std::result::Result<Vec<_>, _>>()?;
        let raw: Option<String> = tx
            .query_row(
                "SELECT spec_json FROM verb_specs WHERE lexeme_id=?1",
                [id],
                |r| r.get(0),
            )
            .optional()?;
        let verb: Option<Verb> = raw.map(|s| serde_json::from_str(&s)).transpose()?;
        let v = serde_json::to_value(&verb)?;
        if (tables != rule.previous_tables && tables != rule.tables)
            || (parts != rule.previous_principal_parts && parts != rule.principal_parts)
            || (v != serde_json::to_value(&rule.previous_verb)?
                && v != serde_json::to_value(&rule.verb)?)
        {
            return Err(format!("古典語形への移行を中止しました: {} に個別編集があります。データは変更していません。",rule.lemma).into());
        }
        if tables != rule.tables {
            tx.execute("DELETE FROM paradigms WHERE lexeme_id=?1", [id])?;
            for (ordinal, table) in rule.tables.iter().enumerate() {
                tx.execute(
                    "INSERT INTO paradigms VALUES(?1,?2,?3)",
                    params![id, ordinal as i64, serde_json::to_string(table)?],
                )?;
            }
        }
        if let Some(verb) = rule.verb {
            tx.execute(
                "UPDATE verb_specs SET spec_json=?1 WHERE lexeme_id=?2",
                params![serde_json::to_string(&verb)?, id],
            )?;
        }
        tx.execute(
            "UPDATE lexemes SET principal_parts=?1 WHERE id=?2",
            params![rule.principal_parts, id],
        )?;
    }
    tx.execute(
        "INSERT OR REPLACE INTO metadata VALUES('morphology_edition',?1)",
        [profile.edition],
    )?;
    tx.commit()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn upgrades_old_data_without_losing_progress_and_is_idempotent() -> Result<()> {
        let mut db = crate::Database::initialize(Connection::open_in_memory()?)?;
        for r in rules()? {
            let id: i64 = db.connection.query_row(
                "SELECT id FROM lexemes WHERE lemma=?1 AND pos=?2",
                params![r.lemma, r.pos],
                |row| row.get(0),
            )?;
            db.connection
                .execute("DELETE FROM paradigms WHERE lexeme_id=?1", [id])?;
            for (i, t) in r.previous_tables.iter().enumerate() {
                db.connection.execute(
                    "INSERT INTO paradigms VALUES(?1,?2,?3)",
                    params![id, i as i64, serde_json::to_string(t)?],
                )?;
            }
            if let Some(v) = r.previous_verb {
                db.connection.execute(
                    "UPDATE verb_specs SET spec_json=?1 WHERE lexeme_id=?2",
                    params![serde_json::to_string(&v)?, id],
                )?;
            }
            db.connection.execute(
                "UPDATE lexemes SET principal_parts=?1 WHERE id=?2",
                params![r.previous_principal_parts, id],
            )?;
        }
        db.connection
            .execute("DELETE FROM metadata WHERE key='morphology_edition'", [])?;
        db.set_learned(1, true)?;
        db.connection
            .execute("INSERT INTO game_records VALUES('existing',1234,5)", [])?;
        migrate(&mut db.connection)?;
        migrate(&mut db.connection)?;
        let words = db.words()?;
        assert_eq!(words.len(), 207);
        assert!(words[0].learned);
        let serialized = serde_json::to_string(&words)?;
        for forbidden in [
            "fēminābus",
            "ossua",
            "ossuum",
            "quoius",
            "hibus",
            "tetinī",
            "pepugī",
            "ēssum",
            " + present active",
        ] {
            assert!(!serialized.contains(forbidden), "{forbidden}");
        }
        let femina = words.iter().find(|w| w.lemma == "fēmina").unwrap();
        assert_eq!(femina.tables[0][3][2], "fēminīs");
        assert_eq!(femina.tables[0][5][2], "fēminīs");
        let Some(Verb::Regular(edo)) = &words.iter().find(|w| w.lemma == "edō").unwrap().verb
        else {
            panic!()
        };
        assert!(edo.generate()?["indicative.passive.perfect.1s"].contains(&"ēsus sum".into()));
        assert_eq!(
            db.connection.query_row(
                "SELECT best_score FROM game_records WHERE rules_key='existing'",
                [],
                |r| r.get::<_, u32>(0)
            )?,
            1234
        );
        let config = crate::game::Config {
            direction: crate::game::Direction::Inflection,
            ..Default::default()
        };
        let pool = crate::game::question_pool(&words, &config)?;
        let q = pool
            .iter()
            .find(|q| q.prompt == "fēmina" && q.clue.contains("複数・与格"))
            .unwrap();
        assert!(q.accepts("feminis", false));
        assert!(!q.accepts("feminabus", false));
        Ok(())
    }
    #[test]
    fn refuses_unrecognized_edits_atomically() -> Result<()> {
        let mut db = crate::Database::initialize(Connection::open_in_memory()?)?;
        db.connection
            .execute("DELETE FROM metadata WHERE key='morphology_edition'", [])?;
        db.connection.execute(
            "UPDATE lexemes SET principal_parts='custom edit' WHERE lemma='fēmina'",
            [],
        )?;
        assert!(migrate(&mut db.connection).is_err());
        assert_eq!(
            db.connection.query_row(
                "SELECT COUNT(*) FROM metadata WHERE key='morphology_edition'",
                [],
                |r| r.get::<_, u32>(0)
            )?,
            0
        );
        assert_eq!(
            db.connection.query_row(
                "SELECT principal_parts FROM lexemes WHERE lemma='fēmina'",
                [],
                |r| r.get::<_, String>(0)
            )?,
            "custom edit"
        );
        Ok(())
    }
}
