mod copy_range;
mod copy_ui;
mod game_ui;
mod music;
use eframe::egui;
use latin_vocab::{Database, Word};
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let mut path = None;
    let mut check = false;
    let mut game_tab = false;
    let mut copy_tab = false;
    let mut play = false;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--db" => path = Some(PathBuf::from(args.next().ok_or("--db requires a path")?)),
            "--check" => check = true,
            "--copy" => copy_tab = true,
            "--game" => game_tab = true,
            "--play" => {
                game_tab = true;
                play = true;
            }
            "--help" | "-h" => {
                println!(
                    "latin-vocab [--db PATH] [--check] [--game | --play | --copy]\n--check: initialize and verify the database without opening a window"
                );
                return Ok(());
            }
            _ => return Err(format!("Unknown argument: {arg}").into()),
        }
    }
    let path = path.unwrap_or_else(|| {
        directories::ProjectDirs::from("org", "LatinVocab", "latin-vocab")
            .expect("Cannot determine application data directory; use --db PATH")
            .data_local_dir()
            .join("vocabulary.sqlite3")
    });
    let db = Database::open(&path)?;
    let words = db.words()?;
    if check {
        println!(
            "{} concepts, {} stored paradigms, {} generated verbs; database: {}",
            words.len(),
            words.iter().filter(|w| !w.tables.is_empty()).count(),
            words
                .iter()
                .filter(|w| matches!(w.verb, Some(latin_vocab::morphology::Verb::Regular(_))))
                .count(),
            path.display()
        );
        return Ok(());
    }
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1120.0, 780.0])
            .with_min_inner_size([760.0, 520.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Latina — ラテン語単語帳",
        options,
        Box::new(move |cc| {
            configure_ui(&cc.egui_ctx);
            let mut game = game_ui::Screen::default();
            if play {
                game.start(&words, &db);
            }
            Ok(Box::new(App {
                copy_tab,
                copy: copy_ui::Screen::default(),
                game_tab,
                game,
                db,
                words,
                path,
                query: String::new(),
                selected: Some(0),
                study: false,
                reverse: false,
                revealed: false,
                unlearned: false,
                error: String::new(),
            }))
        }),
    )?;
    Ok(())
}

fn configure_ui(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    fonts.font_data.insert(
        "japanese".into(),
        egui::FontData::from_static(include_bytes!("../assets/NotoSansCJK-Regular.ttc")).into(),
    );
    fonts
        .families
        .entry(egui::FontFamily::Proportional)
        .or_default()
        .push("japanese".into());
    fonts.font_data.insert(
        "arcade".into(),
        egui::FontData::from_static(include_bytes!("../assets/NotoSansCJK-Bold.ttc")).into(),
    );
    fonts.families.insert(
        egui::FontFamily::Name("arcade".into()),
        vec!["arcade".into()],
    );
    ctx.set_fonts(fonts);
    ctx.set_visuals(egui::Visuals::light());
}

struct App {
    copy_tab: bool,
    copy: copy_ui::Screen,
    game_tab: bool,
    game: game_ui::Screen,
    db: Database,
    words: Vec<Word>,
    path: PathBuf,
    query: String,
    selected: Option<usize>,
    study: bool,
    reverse: bool,
    revealed: bool,
    unlearned: bool,
    error: String,
}

impl App {
    fn filtered(&self) -> Vec<usize> {
        self.words
            .iter()
            .enumerate()
            .filter(|(_, w)| (!self.unlearned || !w.learned) && w.matches(&self.query))
            .map(|(i, _)| i)
            .collect()
    }
    fn next(&mut self) {
        let list = self.filtered();
        self.selected = list
            .iter()
            .copied()
            .find(|i| Some(*i) > self.selected)
            .or_else(|| list.first().copied());
        self.revealed = false;
    }
    fn mark(&mut self, index: usize, learned: bool) {
        match self.db.set_learned(self.words[index].number, learned) {
            Ok(()) => {
                self.words[index].learned = learned;
                self.error.clear();
                if self.study {
                    self.next();
                }
            }
            Err(e) => self.error = format!("保存できませんでした: {e}"),
        }
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        #[cfg(feature = "screenshot")]
        if let Ok(path) = std::env::var("LATINA_SCREENSHOT_TO") {
            // Capture a focused play fixture even when the desktop keeps terminal focus.
            ui.input_mut(|i| i.focused = true);
            let capture = ui.input(|i| {
                i.events.iter().find_map(|event| {
                    if let egui::Event::Screenshot { image, .. } = event {
                        Some(image.clone())
                    } else {
                        None
                    }
                })
            });
            if let Some(capture) = capture {
                let bytes: Vec<u8> = capture.pixels.iter().flat_map(|p| p.to_array()).collect();
                image::save_buffer(
                    &path,
                    &bytes,
                    capture.width() as u32,
                    capture.height() as u32,
                    image::ColorType::Rgba8,
                )
                .expect("save screenshot");
                ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
            } else if ui.ctx().cumulative_pass_nr() >= 3 {
                ui.ctx()
                    .send_viewport_cmd(egui::ViewportCommand::Screenshot(Default::default()));
            }
            ui.ctx().request_repaint();
        }

        self.draw(ui);
        self.game.music.update(
            self.game_tab
                && !self.copy_tab
                && ui.input(|i| i.focused)
                && !self.game.run.as_ref().is_some_and(|r| r.paused),
            self.game
                .run
                .as_ref()
                .is_some_and(|r| r.phase != latin_vocab::game::Phase::Finished),
        );
    }
}

impl App {
    fn draw(&mut self, ui: &mut egui::Ui) {
        if self.copy_tab {
            if self.copy.draw(ui, &self.words) {
                self.copy_tab = false;
            }
            return;
        }
        if self.game_tab {
            match self.game.draw(ui, &self.words, &self.db) {
                game_ui::Action::None => {}
                game_ui::Action::Library => self.game_tab = false,
                game_ui::Action::Review(number) => {
                    self.game_tab = false;
                    self.study = false;
                    self.query.clear();
                    self.unlearned = false;
                    self.selected = self.words.iter().position(|w| w.number == number);
                }
            }
            return;
        }

        let shortcut = |key| ui.input_mut(|i| i.consume_key(egui::Modifiers::CTRL, key));
        if shortcut(egui::Key::Num1) {
            self.study = false;
        }
        if shortcut(egui::Key::Num2) {
            self.study = true;
            self.revealed = false;
        }
        if shortcut(egui::Key::Num3) {
            self.game_tab = true;
            self.game.enter();
        }
        if shortcut(egui::Key::Num4) {
            self.copy_tab = true;
            self.copy
                .enter(&self.words, self.selected.map(|i| self.words[i].number));
        }
        if shortcut(egui::Key::F) {
            ui.memory_mut(|m| m.request_focus(egui::Id::new("word_search")));
        }
        if shortcut(egui::Key::U) {
            self.unlearned = !self.unlearned;
        }
        if shortcut(egui::Key::R) {
            self.reverse = !self.reverse;
            self.revealed = false;
        }
        let move_word = if shortcut(egui::Key::ArrowDown) {
            1
        } else if shortcut(egui::Key::ArrowUp) {
            -1
        } else {
            0
        };
        if move_word != 0 {
            let list = self.filtered();
            if !list.is_empty() {
                let at = list
                    .iter()
                    .position(|i| Some(*i) == self.selected)
                    .unwrap_or(0);
                self.selected =
                    Some(list[(at as isize + move_word).rem_euclid(list.len() as isize) as usize]);
                self.revealed = false;
            }
        }
        if shortcut(egui::Key::Enter) && self.study {
            self.revealed = true;
        }
        if let Some(index) = self.selected {
            if shortcut(egui::Key::K) {
                self.mark(index, true);
            } else if shortcut(egui::Key::J) {
                self.mark(index, false);
            }
        }
        let scroll = if shortcut(egui::Key::PageDown) {
            -350.0
        } else if shortcut(egui::Key::PageUp) {
            350.0
        } else {
            0.0
        };
        let horizontal_scroll = if shortcut(egui::Key::ArrowRight) {
            -200.0
        } else if shortcut(egui::Key::ArrowLeft) {
            200.0
        } else {
            0.0
        };
        egui::Panel::top("header").show(ui, |ui| {
            ui.add_space(12.0);
            ui.horizontal_wrapped(|ui| {
                ui.heading(
                    egui::RichText::new("Latina")
                        .size(30.0)
                        .color(egui::Color32::from_rgb(36, 89, 75)),
                );
                ui.label("ラテン語単語帳");
                ui.separator();
                if ui.selectable_label(!self.study, "単語一覧").clicked() {
                    self.study = false;
                }
                if ui.selectable_label(self.study, "学習する").clicked() {
                    self.study = true;
                    self.revealed = false;
                }
                if ui.selectable_label(false, "ゲームで遊ぶ").clicked() {
                    self.game_tab = true;
                    self.game.enter();
                }
                if ui.selectable_label(false, "写経練習").clicked() {
                    self.copy_tab = true;
                    self.copy
                        .enter(&self.words, self.selected.map(|i| self.words[i].number));
                }
                ui.separator();
                ui.label(format!(
                    "{} / {} 習得",
                    self.words.iter().filter(|w| w.learned).count(),
                    self.words.len()
                ));
            });
            ui.add_space(12.0);
        });
        egui::Panel::bottom("footer").show(ui, |ui| {
            ui.small(
                "Tab / Shift+Tab 選択・Enter 決定 ｜ Ctrl+1 一覧 / 2 学習 / 3 ゲーム / 4 写経 / F 検索",
            );
            ui.small(
                "Ctrl+↑↓ 単語移動 / Enter 答え / K 覚えた / J もう一度 / R 出題方向 / U 未習得",
            );
            ui.small("Ctrl+PageUp・PageDown 表の上下移動 / Ctrl+←→ 表の左右移動");
            ui.small(
                "語彙・変化表: Wiktionary contributors / CC BY-SA 4.0 · 日本語訳は本アプリで付与",
            );
            ui.label(
                egui::RichText::new(format!("保存先: {}", self.path.display()))
                    .small()
                    .weak(),
            );
            if !self.error.is_empty() {
                ui.colored_label(egui::Color32::DARK_RED, &self.error);
            }
        });
        egui::Panel::left("words")
            .default_size(270.0)
            .resizable(true)
            .show(ui, |ui| {
                ui.add_space(12.0);
                ui.label("SWADESH · 207");
                let changed = ui
                    .add(
                        egui::TextEdit::singleline(&mut self.query)
                            .id(egui::Id::new("word_search"))
                            .hint_text("ラテン語・日本語で検索"),
                    )
                    .changed()
                    | ui.checkbox(&mut self.unlearned, "未習得のみ").changed();
                let filtered = self.filtered();
                if changed || !self.selected.is_some_and(|i| filtered.contains(&i)) {
                    self.selected = filtered.first().copied();
                    self.revealed = false;
                }
                ui.small(format!("{} 件", filtered.len()));
                ui.separator();
                egui::ScrollArea::vertical().show(ui, |ui| {
                    for i in filtered {
                        let w = &self.words[i];
                        let label = if self.study {
                            format!(
                                "{:03}  {}",
                                w.number,
                                if self.reverse {
                                    &w.meaning_ja
                                } else {
                                    &w.source_form
                                }
                            )
                        } else {
                            format!(
                                "{:03}  {}   {}{}",
                                w.number,
                                w.lemma,
                                w.meaning_ja,
                                if w.learned { "  ✓" } else { "" }
                            )
                        };
                        let response = ui.selectable_label(self.selected == Some(i), label);
                        if move_word != 0 && self.selected == Some(i) {
                            response.scroll_to_me(Some(egui::Align::Center));
                        }
                        if response.clicked() {
                            self.selected = Some(i);
                            self.revealed = false;
                        }
                    }
                });
            });
        egui::CentralPanel::default().show(ui, |ui| {
            egui::ScrollArea::both().show(ui, |ui| {
                ui.scroll_with_delta(egui::vec2(horizontal_scroll, scroll));
                ui.add_space(20.0);
                let Some(index) = self.selected else {
                    ui.heading(if self.unlearned && self.query.is_empty() {
                        "すべての単語を習得しました"
                    } else {
                        "該当する単語がありません"
                    });
                    ui.label("検索条件を変更すると、ほかの単語を表示できます。");
                    return;
                };
                let word = self.words[index].clone();
                ui.small(format!("SWADESH {:03}", word.number));
                if self.study {
                    if ui
                        .checkbox(&mut self.reverse, "日本語 → ラテン語")
                        .changed()
                    {
                        self.revealed = false;
                    }
                    ui.add_space(24.0);
                    ui.heading(
                        egui::RichText::new(if self.reverse {
                            &word.meaning_ja
                        } else {
                            &word.source_form
                        })
                        .size(42.0),
                    );
                    ui.add_space(20.0);
                    if !self.revealed {
                        if ui.button("答えを表示").clicked() {
                            self.revealed = true;
                        }
                        if ui.button("次の単語").clicked() {
                            self.next();
                        }
                        return;
                    }
                }
                ui.heading(egui::RichText::new(&word.lemma).size(38.0));
                ui.label(egui::RichText::new(&word.meaning_ja).size(24.0));
                ui.label(format!("{} · {}", pos_ja(&word.pos), word.concept_en));
                if word.source_form != word.lemma {
                    ui.label(format!(
                        "リスト上の語形: {} ／ 見出し語: {}",
                        word.source_form, word.lemma
                    ));
                }
                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    if self.study {
                        if ui.button("覚えた").clicked() {
                            self.mark(index, true);
                        }
                        if ui.button("もう一度").clicked() {
                            self.mark(index, false);
                        }
                    } else {
                        let mut learned = word.learned;
                        if ui.checkbox(&mut learned, "習得済み").changed() {
                            self.mark(index, learned);
                        }
                    }
                });
                ui.separator();
                ui.label(&word.principal_parts);
                ui.add_space(12.0);
                ui.heading("変化表");
                match word.morphology_status.as_str() {
                    "generated" => {
                        if let Some(latin_vocab::morphology::Verb::Regular(v)) = &word.verb {
                            ui.label(format!("{} · 主要部分から規則生成",v.conjugation.label()));
                            ui.small("個別の例外は辞書データを優先します。複合形には性・数による形を併記します。");
                        }
                    }
                    "invariable" => {
                        ui.label("不変化語です。");
                    }
                    "missing" => {
                        ui.label("変化表は未収録です。出典の辞書項目をご確認ください。");
                    }
                    _ => {
                        ui.small("古典ラテン語の標準的な学習用語形を収録しています。");
                    }
                }
                let tables = match word.display_tables() {
                    Ok(tables) => tables,
                    Err(error) => { ui.colored_label(egui::Color32::DARK_RED, format!("活用を生成できません: {error}")); return; }
                };
                for (t, table) in tables.iter().enumerate() {
                    ui.add_space(8.0);
                    egui::Grid::new(("paradigm", word.number, t))
                        .striped(true)
                        .spacing([18.0, 8.0])
                        .show(ui, |ui| {
                            for row in table {
                                for cell in row {
                                    ui.add(egui::Label::new(grammar_ja(cell)).wrap());
                                }
                                ui.end_row();
                            }
                        });
                }
                ui.add_space(16.0);
                ui.label("出典（URLを選択してコピーできます）");
                let mut source = word.source.clone();
                ui.add(egui::TextEdit::singleline(&mut source).desired_width(650.0));
                ui.small("https://creativecommons.org/licenses/by-sa/4.0/");
            });
        });
    }
}

fn pos_ja(pos: &str) -> &str {
    match pos {
        "Noun" => "名詞",
        "Verb" => "動詞",
        "Adjective" => "形容詞",
        "Pronoun" => "代名詞",
        "Adverb" => "副詞",
        "Numeral" => "数詞",
        "Preposition" => "前置詞",
        "Conjunction" => "接続詞",
        "Determiner" => "限定詞",
        "Particle" => "小辞",
        "Participle" => "分詞",
        _ => pos,
    }
}
fn grammar_ja(s: &str) -> &str {
    match s {
        "singular" => "単数",
        "plural" => "複数",
        "nominative" => "主格",
        "genitive" => "属格",
        "dative" => "与格",
        "accusative" => "対格",
        "ablative" => "奪格",
        "vocative" => "呼格",
        "locative" => "所格",
        "indicative" => "直説法",
        "subjunctive" => "接続法",
        "imperative" => "命令法",
        "active" => "能動態",
        "passive" => "受動態",
        "present" => "現在",
        "imperfect" => "未完了過去",
        "future" => "未来",
        "perfect" => "完了",
        "pluperfect" => "過去完了",
        "future perfect" => "未来完了",
        "first" => "一人称",
        "second" => "二人称",
        "third" => "三人称",
        "masculine" => "男性",
        "feminine" => "女性",
        "neuter" => "中性",
        _ => s,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn renders_dictionary_and_both_card_directions() -> latin_vocab::Result<()> {
        let path =
            std::env::temp_dir().join(format!("latin-vocab-ui-{}.sqlite3", std::process::id()));
        let db = Database::open(&path)?;
        let words = db.words()?;
        let mut app = App {
            copy_tab: false,
            copy: copy_ui::Screen::default(),
            game_tab: false,
            game: game_ui::Screen::default(),
            db,
            words,
            path: path.clone(),
            query: String::new(),
            selected: Some(149),
            study: false,
            reverse: false,
            revealed: false,
            unlearned: false,
            error: String::new(),
        };
        let ctx = egui::Context::default();
        configure_ui(&ctx);
        for (index, study, reverse, revealed) in [
            (149, false, false, false),
            (119, false, false, false),
            (0, true, false, false),
            (0, true, true, true),
        ] {
            app.selected = Some(index);
            app.study = study;
            app.reverse = reverse;
            app.revealed = revealed;
            let mut output = ctx.run_ui(egui::RawInput::default(), |ui| app.draw(ui));
            assert!(!output.shapes.is_empty());
            output.textures_delta.clear();
        }
        app.unlearned = true;
        app.selected = Some(149);
        app.mark(149, true);
        assert!(!app.filtered().contains(&149));
        assert_ne!(app.selected, Some(149));
        drop(app);
        std::fs::remove_file(path)?;
        Ok(())
    }
}
