use latin_vocab::{Database, Word, game::*};
fn words() -> Vec<Word> {
    serde_json::from_value(serde_json::from_str::<serde_json::Value>(include_str!("../data/swadesh.json")).unwrap()["words"].clone()).unwrap()
}
fn run() -> Run {
    Run::start(
        question_pool(&words(), &Config::default()).unwrap(),
        Config::default(),
        42,
    )
    .unwrap()
}
#[test]
fn matching_and_homographs() {
    assert_eq!(answer_key(" ＲĒＧŌ ", false, false), "rego");
    assert_ne!(
        answer_key("rego", false, true),
        answer_key("rēgō", false, true)
    );
    assert_eq!(answer_key("～ ミズ", true, false), "みず");
    let pool = question_pool(
        &words(),
        &Config {
            direction: Direction::LatinToJapanese,
            ..Default::default()
        },
    )
    .unwrap();
    let water = pool.iter().find(|q| q.prompt == "aqua").unwrap();
    assert!(water.accepts("水", false));
    assert!(water.accepts("ミズ", false));
    assert!(!water.accepts("", false));
    assert!(pool.iter().any(|q| q.prompt == "os"));
    assert!(pool.iter().any(|q| q.prompt == "ōs"));
    let long = pool.iter().find(|q| q.prompt == "longus").unwrap();
    assert!(long.accepts("長い", false));
    assert!(long.accepts("遠い", false));
}
#[test]
fn rounds_are_unique_reproducible_and_respect_filter() {
    let a = run();
    let b = run();
    assert_eq!(a.questions.len(), 16);
    assert_eq!(
        a.questions
            .iter()
            .map(|q| q.word_number)
            .collect::<Vec<_>>(),
        b.questions
            .iter()
            .map(|q| q.word_number)
            .collect::<Vec<_>>()
    );
    assert_eq!(
        a.questions
            .iter()
            .map(|q| q.word_number)
            .collect::<std::collections::BTreeSet<_>>()
            .len(),
        16
    );
    let mut w = words();
    for word in &mut w {
        word.learned = true;
    }
    let config = Config {
        unlearned_only: true,
        ..Default::default()
    };
    assert!(Run::start(question_pool(&w, &config).unwrap(), config.clone(), 1).is_none());
    w[0].learned = false;
    assert_eq!(
        Run::start(question_pool(&w, &config).unwrap(), config, 1)
            .unwrap()
            .questions
            .len(),
        1
    );
}
#[test]
fn timeout_pause_and_game_over_are_single_transitions() {
    let mut r = run();
    r.paused = true;
    r.tick(100.);
    r.skip();
    r.submit("wrong");
    assert_eq!(r.lives, 3);
    assert_eq!(r.remaining, 20.);
    r.paused = false;
    r.tick(f32::NAN);
    r.tick(-10.);
    assert_eq!(r.remaining, 20.);
    r.tick(100.);
    r.tick(100.);
    r.submit("wrong");
    r.skip();
    assert_eq!(r.lives, 2);
    assert_eq!(r.attempts.len(), 1);
    assert_eq!(r.attempts[0].outcome, Outcome::Timeout);
    r.advance();
    r.submit("wrong");
    r.advance();
    r.skip();
    r.advance();
    assert_eq!(r.phase, Phase::Finished);
    assert!(!r.cleared());
    assert_eq!(r.lives, 0);
}
#[test]
fn completion_scoring_and_records_preserve_learning() {
    let path = std::env::temp_dir().join(format!("latin-game-test-{}.sqlite3", std::process::id()));
    let db = Database::open(&path).unwrap();
    db.set_learned(1, true).unwrap();
    let mut r = run();
    assert!(db.record_game(&r).is_err());
    for _ in 0..16 {
        let answer = r.current().unwrap().answers[0].clone();
        r.submit(&answer);
        r.advance();
    }
    assert!(r.cleared());
    assert_eq!(r.best_combo, 16);
    assert_eq!(r.score, 3550);
    assert_eq!(db.record_game(&r).unwrap(), 3550);
    drop(db);
    let db = Database::open(&path).unwrap();
    assert_eq!(db.game_best(&r.config).unwrap(), 3550);
    assert_eq!(
        db.game_best(&Config {
            difficulty: Difficulty::Hard,
            ..Default::default()
        })
        .unwrap(),
        0
    );
    assert!(
        db.words()
            .unwrap()
            .iter()
            .find(|w| w.number == 1)
            .unwrap()
            .learned
    );
    assert!(
        db.words()
            .unwrap()
            .iter()
            .filter(|w| matches!(w.verb, Some(latin_vocab::morphology::Verb::Regular(_))))
            .all(|w| w.tables.is_empty())
    );
    drop(db);
    std::fs::remove_file(path).unwrap();
}
#[test]
fn inflection_uses_noun_tables_and_verb_overrides() {
    let pool = question_pool(
        &words(),
        &Config {
            direction: Direction::Inflection,
            ..Default::default()
        },
    )
    .unwrap();
    assert!(pool.iter().any(|q| q.prompt == "aqua"
        && q.clue.contains("複数・属格")
        && q.accepts("aquārum", true)));
    assert!(pool.iter().any(|q| q.prompt == "dīcō"
        && q.clue == "命令法・能動・現在・単数2人称"
        && q.accepts("dīc", true)
        && !q.accepts("dīce", true)));
}

fn answer_correctly(r: &mut Run) {
    let answer = r.current().unwrap().answers[0].clone();
    r.submit(&answer);
    r.advance();
}
#[test]
fn review_returns_after_two_normal_questions_and_restores_one_life() {
    let mut r = run();
    let original = r.current().unwrap().clone();
    r.submit("incorrect");
    r.advance();
    assert_eq!(r.lives, 2);
    for expected in [1, 2] {
        assert_eq!(r.current_index(), expected);
        assert!(!r.is_review());
        answer_correctly(&mut r);
    }
    assert!(r.is_review());
    assert_eq!(r.current_index(), 0);
    assert_eq!(r.current().unwrap().prompt, original.prompt);
    assert_eq!(r.current().unwrap().clue, original.clue);
    let score = r.score;
    let combo = r.combo;
    r.submit(&original.answers[0]);
    assert_eq!(r.lives, 3);
    assert!(r.attempts.last().unwrap().life_recovered);
    assert_eq!(r.score, score);
    assert_eq!(r.combo, combo);
    r.submit(&original.answers[0]);
    assert_eq!(r.lives, 3);
    r.advance();
    assert!(!r.is_review());
    assert_eq!(r.current_index(), 3);
    while r.phase != Phase::Finished {
        answer_correctly(&mut r);
    }
    assert!(r.cleared());
    assert_eq!(r.attempts.len(), 17);
}
#[test]
fn end_of_round_reviews_and_failed_reviews_terminate() {
    let mut r = run();
    for _ in 0..15 {
        answer_correctly(&mut r);
    }
    r.tick(100.);
    r.advance();
    assert!(r.is_review());
    assert_eq!(r.current_index(), 15);
    r.submit("incorrect");
    r.advance();
    assert_eq!(r.phase, Phase::Finished);
    assert_eq!(r.lives, 1);
    assert_eq!(r.attempts.len(), 17);
}
#[test]
fn multiple_reviews_are_ordered_and_skips_do_not_schedule_reviews() {
    let mut r = run();
    r.submit("incorrect");
    r.advance();
    r.tick(100.);
    r.advance();
    answer_correctly(&mut r);
    assert!(r.is_review());
    assert_eq!(r.current_index(), 0);
    answer_correctly(&mut r);
    assert!(!r.is_review());
    assert_eq!(r.current_index(), 3);
    answer_correctly(&mut r);
    assert!(r.is_review());
    assert_eq!(r.current_index(), 1);
    answer_correctly(&mut r);
    assert_eq!(r.lives, 3);
    let mut r = run();
    r.skip();
    r.advance();
    while r.phase != Phase::Finished {
        answer_correctly(&mut r);
    }
    assert_eq!(r.attempts.len(), 16);
    assert_eq!(r.lives, 2);
}
