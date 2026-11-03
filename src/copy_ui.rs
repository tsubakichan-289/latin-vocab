//! Untimed copy practice. Generated paradigms and input drafts remain in memory.
use eframe::egui::{self, Color32, Key};
use latin_vocab::{Word, game::answer_key, morphology::Table};
use std::collections::BTreeMap;

struct Cell {
    row: usize,
    col: usize,
    model: String,
    input: String,
    correct: Option<bool>,
}
struct Exercise {
    table: Table,
    cells: Vec<Cell>,
    noun: bool,
}
impl Exercise {
    fn new(table: Table, noun: bool) -> Self {
        let cells = table
            .iter()
            .enumerate()
            .skip(1)
            .flat_map(|(row, values)| {
                values
                    .iter()
                    .enumerate()
                    .skip(1)
                    .filter(|(_, value)| {
                        !value.trim().is_empty() && !matches!(value.trim(), "—" | "–" | "-" | "?")
                    })
                    .map(move |(col, value)| Cell {
                        row,
                        col,
                        model: value.clone(),
                        input: String::new(),
                        correct: None,
                    })
            })
            .collect();
        Self { table, cells, noun }
    }
    fn check(&mut self, index: usize, strict: bool) {
        let c = &mut self.cells[index];
        let key = answer_key(&c.input, false, strict);
        let matches = !key.is_empty()
            && (key == answer_key(&c.model, false, strict)
                || c.model
                    .split(['/', ',', '、'])
                    .flat_map(|s| {
                        if self.noun {
                            s.split_whitespace().collect::<Vec<_>>()
                        } else {
                            vec![s]
                        }
                    })
                    .any(|s| key == answer_key(s, false, strict)));
        c.correct = Some(matches);
    }
    fn completed(&self) -> usize {
        self.cells
            .iter()
            .filter(|c| c.correct == Some(true))
            .count()
    }
}
#[derive(Default)]
pub struct Screen {
    range: crate::copy_range::Range,
    random_exercise: Option<Exercise>,
    show_models: bool,
    selected: Option<u32>,
    table_index: usize,
    tables: Vec<Table>,
    exercises: BTreeMap<(u32, usize), Exercise>,
    query: String,
    strict: bool,
    focus: Option<usize>,
    composing: bool,
    error: String,
}
impl Screen {
    pub fn enter(&mut self, words: &[Word], preferred: Option<u32>) {
        if self.selected.is_none() {
            let eligible = |w: &&Word| matches!(w.pos.as_str(), "Noun" | "Verb");
            if let Some(w) = words
                .iter()
                .filter(eligible)
                .find(|w| Some(w.number) == preferred)
                .or_else(|| words.iter().find(eligible))
            {
                self.select(w);
            }
        }
    }
    fn select(&mut self, word: &Word) {
        self.random_exercise = None;
        self.selected = Some(word.number);
        self.table_index = 0;
        self.tables = match word.display_tables() {
            Ok(tables) => {
                self.error.clear();
                tables
            }
            Err(e) => {
                self.error = format!("変化表を読み込めません: {e}");
                vec![]
            }
        };
        self.focus = None;
        self.composing = false;
    }
    fn change_table(&mut self, delta: isize) {
        if !self.tables.is_empty() {
            self.table_index =
                (self.table_index as isize + delta).rem_euclid(self.tables.len() as isize) as usize;
            self.focus = None;
            self.composing = false;
        }
    }
    fn random_question(&mut self, words: &[Word], mut seed: u64) {
        use crate::copy_range::random;
        let mut candidates = match self.range.candidates(words) {
            Ok(c) => c,
            Err(e) => {
                self.error = format!("出題できません: {e}");
                return;
            }
        };
        if candidates.is_empty() {
            self.error = "この範囲には出題できる表がありません。範囲を広げてください。".into();
            return;
        }
        if candidates.len() > 1 {
            candidates.retain(|(w, _)| Some(w.number) != self.selected);
        }
        let (word, tables) = candidates.swap_remove(random(&mut seed, candidates.len()));
        let table = tables[random(&mut seed, tables.len())].clone();
        let mut exercise = Exercise::new(table.clone(), word.pos == "Noun");
        // Keep at least one given form whenever a table has multiple cells.
        let count = self
            .range
            .blanks
            .max(1)
            .min(exercise.cells.len().saturating_sub(1).max(1));
        for i in (1..exercise.cells.len()).rev() {
            let j = random(&mut seed, i + 1);
            exercise.cells.swap(i, j);
        }
        exercise.cells.truncate(count);
        exercise.cells.sort_by_key(|c| (c.row, c.col));
        self.select(word);
        self.tables = vec![table];
        self.table_index = 0;
        self.random_exercise = Some(exercise);
        self.focus = Some(0);
    }
    /// Returns true when the user requests the library.
    pub fn draw(&mut self, ui: &mut egui::Ui, words: &[Word]) -> bool {
        self.enter(words, None);
        apply_theme(ui);
        let ctrl = |k| ui.input_mut(|i| i.consume_key(egui::Modifiers::CTRL, k));
        let back = ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, Key::Escape));
        let check_all = ctrl(Key::Enter);
        let focus_input = ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, Key::F2));
        let focus_search = ctrl(Key::F);
        let word_delta = if ctrl(Key::ArrowDown) {
            1
        } else if ctrl(Key::ArrowUp) {
            -1
        } else {
            0
        };
        let table_delta = if ctrl(Key::ArrowRight) {
            1
        } else if ctrl(Key::ArrowLeft) {
            -1
        } else {
            0
        };
        let scroll = if ctrl(Key::PageDown) {
            -320.0
        } else if ctrl(Key::PageUp) {
            320.0
        } else {
            0.0
        };
        if table_delta != 0 {
            self.change_table(table_delta);
        }
        let mut leave = back;
        let mut next_random = ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, Key::F3));
        egui::Panel::top("copy_header")
            .frame(panel_frame())
            .show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    title(ui);
                    ui.label(
                        egui::RichText::new("GAME 02  /  COPY & LEARN")
                            .color(GOLD)
                            .strong(),
                    );
                    if ui.button("Esc  単語帳へ").clicked() {
                        leave = true;
                    }
                });
                ui.label(
                    egui::RichText::new(
                        "見て、書いて、覚える。  金色のお手本を、下の空欄に写していこう。",
                    )
                    .color(MUTED),
                );
            });
        egui::Panel::top("copy_range")
            .frame(panel_frame())
            .show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    next_random |= ui.button("F3  範囲からランダム出題").clicked();
                    ui.add(egui::Slider::new(&mut self.range.blanks, 1..=24).text("空欄数"));
                    ui.checkbox(&mut self.show_models, "空欄のお手本も表示");
                    ui.label("範囲の変更は次のランダム出題から反映");
                });
                egui::CollapsingHeader::new("出題範囲を選択")
                    .default_open(false)
                    .show(ui, |ui| {
                        ui.horizontal_wrapped(|ui| {
                            ui.checkbox(&mut self.range.nouns, "名詞");
                            for (i, enabled) in self.range.declensions.iter_mut().enumerate() {
                                ui.checkbox(enabled, format!("第{}変化", i + 1));
                            }
                        });
                        ui.horizontal_wrapped(|ui| {
                            ui.checkbox(&mut self.range.verbs, "動詞");
                            for (i, enabled) in self.range.tenses.iter_mut().enumerate() {
                                ui.checkbox(enabled, crate::copy_range::TENSES[i]);
                            }
                        });
                        ui.horizontal_wrapped(|ui| {
                            for (i, enabled) in self.range.moods.iter_mut().enumerate() {
                                ui.checkbox(enabled, crate::copy_range::MOODS[i]);
                            }
                            ui.checkbox(&mut self.range.voices[0], "能動態・異態");
                            ui.checkbox(&mut self.range.voices[1], "受動態");
                        });
                    });
            });
        if next_random {
            let seed = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos() as u64;
            self.random_question(words, seed);
        }
        egui::Panel::bottom("copy_help").frame(panel_frame()).show(ui, |ui| {
            ui.small(
                "F3 ランダム出題 ｜ F2 入力欄へ ｜ Enter 判定して次の空欄へ ｜ Tab / Shift+Tab 項目移動 ｜ Ctrl+Enter 全欄を判定",
            );
            ui.small(
                "Ctrl+↑↓ 単語 / ←→ 表の切替 ｜ Ctrl+F 検索 ｜ Ctrl+PageUp・PageDown 表のスクロール",
            );
            ui.small("制限時間なし・ライフなし。入力はこの起動中、単語・表ごとに保持します。");
        });
        egui::Panel::left("copy_words")
            .default_size(225.0)
            .frame(panel_frame())
            .show(ui, |ui| {
                ui.label(
                    egui::RichText::new("01  手動で単語を選ぶ")
                        .color(GOLD)
                        .strong(),
                );
                ui.add_space(8.0);
                let response = ui
                    .add(egui::TextEdit::singleline(&mut self.query).hint_text("名詞・動詞を検索"));
                if focus_search {
                    response.request_focus();
                }
                let eligible: Vec<_> = words
                    .iter()
                    .filter(|w| matches!(w.pos.as_str(), "Noun" | "Verb") && w.matches(&self.query))
                    .collect();
                if word_delta != 0 && !eligible.is_empty() {
                    let at = eligible
                        .iter()
                        .position(|w| Some(w.number) == self.selected)
                        .unwrap_or(0);
                    self.select(
                        eligible[(at as isize + word_delta).rem_euclid(eligible.len() as isize)
                            as usize],
                    );
                }
                ui.label(format!("{}語", eligible.len()));
                egui::ScrollArea::vertical().show(ui, |ui| {
                    for word in eligible {
                        let selected = self.selected == Some(word.number);
                        let r = ui.add(
                            egui::Button::new(
                                egui::RichText::new(format!("{}  {}", word.lemma, word.meaning_ja))
                                    .color(if selected { GOLD } else { WHITE }),
                            )
                            .fill(if selected {
                                egui::Color32::from_rgb(63, 55, 34)
                            } else {
                                CARD
                            })
                            .stroke(egui::Stroke::new(1.0, if selected { GOLD } else { EDGE }))
                            .min_size(egui::vec2(ui.available_width(), 36.0)),
                        );
                        ui.add_space(3.0);
                        if word_delta != 0 && self.selected == Some(word.number) {
                            r.scroll_to_me(Some(egui::Align::Center));
                        }
                        if r.clicked() {
                            self.select(word);
                        }
                    }
                });
            });
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(BG).inner_margin(18))
            .show(ui, |ui| {
                let rect = ui.max_rect();
                let painter = ui.painter();
                let step = 64.0;
                for row in 0..(rect.height() / step).ceil() as usize {
                    for col in 0..(rect.width() / step).ceil() as usize {
                        let tile = egui::Rect::from_min_size(
                            rect.min + egui::vec2(col as f32 * step, row as f32 * step),
                            egui::vec2(step, step),
                        );
                        painter.rect_filled(
                            tile.intersect(rect),
                            0.0,
                            if (row + col) % 2 == 0 {
                                BG
                            } else {
                                egui::Color32::from_rgb(28, 29, 44)
                            },
                        );
                    }
                }
                let Some(word) = words.iter().find(|w| Some(w.number) == self.selected) else {
                    ui.label("練習できる名詞・動詞がありません。");
                    return;
                };
                ui.label(
                    egui::RichText::new("02  お手本を写そう")
                        .color(MUTED)
                        .strong(),
                );
                ui.horizontal_wrapped(|ui| {
                    ui.label(
                        egui::RichText::new(&word.lemma)
                            .size(34.0)
                            .color(GOLD)
                            .strong(),
                    );
                    ui.label(
                        egui::RichText::new(&word.meaning_ja)
                            .size(22.0)
                            .color(WHITE),
                    );
                    ui.label(
                        egui::RichText::new(if word.pos == "Noun" {
                            "NOUN / 曲用"
                        } else {
                            "VERB / 活用"
                        })
                        .color(MUTED),
                    );
                });
                ui.label(egui::RichText::new(&word.principal_parts).color(MUTED));
                ui.add_space(8.0);
                if !self.error.is_empty() {
                    ui.colored_label(RED, &self.error);
                }
                if self.tables.is_empty() {
                    ui.label("変化表がありません。");
                    return;
                }
                ui.horizontal_wrapped(|ui| {
                    if ui.button("← 前の表").clicked() {
                        self.change_table(-1);
                    }
                    egui::ComboBox::from_id_salt("copy_table")
                        .selected_text(format!(
                            "表 {} / {}",
                            self.table_index + 1,
                            self.tables.len()
                        ))
                        .show_ui(ui, |ui| {
                            for (i, table) in self.tables.iter().enumerate() {
                                let label = table
                                    .first()
                                    .and_then(|r| r.first())
                                    .filter(|s| !s.is_empty())
                                    .map(String::as_str)
                                    .unwrap_or("曲用表");
                                if ui
                                    .selectable_value(
                                        &mut self.table_index,
                                        i,
                                        format!("{}: {}", i + 1, label),
                                    )
                                    .changed()
                                {
                                    self.focus = None;
                                }
                            }
                        });
                    if ui.button("次の表 →").clicked() {
                        self.change_table(1);
                    }
                });
                if word.pos == "Verb"
                    && let Some(title) = self.tables[self.table_index]
                        .first()
                        .and_then(|r| r.first())
                {
                    ui.label(egui::RichText::new(title).strong().color(GOLD));
                }
                let key = (word.number, self.table_index);
                if self.random_exercise.is_none() {
                    self.exercises.entry(key).or_insert_with(|| {
                        Exercise::new(self.tables[self.table_index].clone(), word.pos == "Noun")
                    });
                }
                ui.horizontal_wrapped(|ui| {
                    if ui
                        .checkbox(&mut self.strict, "長音記号を厳密に判定")
                        .changed()
                    {
                        for e in self
                            .exercises
                            .values_mut()
                            .chain(self.random_exercise.iter_mut())
                        {
                            for c in &mut e.cells {
                                c.correct = None;
                            }
                        }
                    }
                });
                let is_random = self.random_exercise.is_some();
                let exercise = self
                    .random_exercise
                    .as_mut()
                    .unwrap_or_else(|| self.exercises.get_mut(&key).unwrap());
                let mut check = check_all;
                ui.horizontal_wrapped(|ui| {
                    check |= ui
                        .add(
                            egui::Button::new(
                                egui::RichText::new("Ctrl+Enter  全欄を判定").strong(),
                            )
                            .fill(egui::Color32::from_rgb(37, 108, 92)),
                        )
                        .clicked();
                    if ui.button("この表をやり直す").clicked() {
                        for c in &mut exercise.cells {
                            c.input.clear();
                            c.correct = None;
                        }
                        self.focus = Some(0);
                    }
                    if ui.button("F2  入力欄へ").clicked() || focus_input {
                        self.focus = Some(
                            exercise
                                .cells
                                .iter()
                                .position(|c| c.correct != Some(true))
                                .unwrap_or(0),
                        );
                    }
                });
                if check {
                    for i in 0..exercise.cells.len() {
                        exercise.check(i, self.strict);
                    }
                }
                let count = exercise.completed();
                ui.add(
                    egui::ProgressBar::new(count as f32 / exercise.cells.len().max(1) as f32)
                        .fill(GOLD)
                        .desired_height(22.0)
                        .text(format!(
                            "COMPLETE   {} / {} マス",
                            count,
                            exercise.cells.len()
                        )),
                );
                if !exercise.cells.is_empty() && count == exercise.cells.len() {
                    ui.colored_label(GREEN, "完了！ F3で次のランダム問題へ進めます。");
                }
                ui.add_space(12.0);
                let column_count = exercise
                    .table
                    .first()
                    .map_or(2, |row| row.len())
                    .saturating_sub(1)
                    .max(1);
                let cell_width = ((ui.available_width() - 115.0) / column_count as f32 - 28.0)
                    .clamp(170.0, 320.0);
                let mut commit = false;
                for event in ui.input(|i| i.events.clone()) {
                    match event {
                        egui::Event::Ime(egui::ImeEvent::Preedit { text, .. }) => {
                            self.composing = !text.is_empty()
                        }
                        egui::Event::Ime(egui::ImeEvent::Commit(_)) => {
                            commit = true;
                            self.composing = false;
                        }
                        _ => {}
                    }
                }
                let mut submitted = None;
                egui::ScrollArea::both()
                    .id_salt("copy_grid_scroll")
                    .show(ui, |ui| {
                        ui.scroll_with_delta(egui::vec2(0.0, scroll));
                        egui::Grid::new(("copy_grid", key))
                            .striped(false)
                            .spacing([10.0, 10.0])
                            .show(ui, |ui| {
                                for (row, values) in exercise.table.iter().enumerate() {
                                    for (col, value) in values.iter().enumerate() {
                                        if row == 0 || col == 0 {
                                            ui.label(
                                                egui::RichText::new(
                                                    if row == 0
                                                        && col == 0
                                                        && !exercise.noun
                                                        && values.len() == 3
                                                    {
                                                        "人称"
                                                    } else {
                                                        crate::grammar_ja(value)
                                                    },
                                                )
                                                .strong()
                                                .color(MUTED),
                                            );
                                        } else if let Some(index) = exercise
                                            .cells
                                            .iter()
                                            .position(|c| c.row == row && c.col == col)
                                        {
                                            let cell = &mut exercise.cells[index];
                                            let focused = ui.memory(|m| {
                                                m.has_focus(egui::Id::new((
                                                    "copy_input",
                                                    key,
                                                    index,
                                                )))
                                            });
                                            let border = if focused {
                                                GOLD
                                            } else if cell.correct == Some(true) {
                                                GREEN
                                            } else if cell.correct == Some(false) {
                                                RED
                                            } else {
                                                EDGE
                                            };
                                            egui::Frame::new()
                                                .fill(CARD)
                                                .corner_radius(8)
                                                .inner_margin(10)
                                                .stroke(egui::Stroke::new(
                                                    if focused { 2.0 } else { 1.0 },
                                                    border,
                                                ))
                                                .show(ui, |ui| {
                                                    ui.vertical(|ui| {
                                                        ui.set_width(cell_width);
                                                        ui.label(
                                                            egui::RichText::new(
                                                                if !is_random
                                                                    || self.show_models
                                                                    || cell.correct == Some(true)
                                                                {
                                                                    &cell.model
                                                                } else {
                                                                    "？ 空欄を埋めよう"
                                                                },
                                                            )
                                                            .color(GOLD)
                                                            .size(19.0)
                                                            .strong(),
                                                        );
                                                        let id = egui::Id::new((
                                                            "copy_input",
                                                            key,
                                                            index,
                                                        ));
                                                        let enter = ui.memory(|m| m.has_focus(id))
                                                            && !commit
                                                            && !self.composing
                                                            && ui.input_mut(|i| {
                                                                i.consume_key(
                                                                    egui::Modifiers::NONE,
                                                                    Key::Enter,
                                                                )
                                                            });
                                                        let response = ui.add(
                                                            egui::TextEdit::singleline(
                                                                &mut cell.input,
                                                            )
                                                            .id(id)
                                                            .desired_width(cell_width)
                                                            .font(egui::FontId::proportional(18.0))
                                                            .text_color(WHITE)
                                                            .hint_text("ここに写す"),
                                                        );
                                                        if response.changed() {
                                                            cell.correct = None;
                                                        }
                                                        if self.focus == Some(index) {
                                                            response.request_focus();
                                                            response.scroll_to_me(Some(
                                                                egui::Align::Center,
                                                            ));
                                                            self.focus = None;
                                                        } else if response.gained_focus() {
                                                            response.scroll_to_me(Some(
                                                                egui::Align::Center,
                                                            ));
                                                        }
                                                        if enter {
                                                            submitted = Some(index);
                                                        }
                                                        match cell.correct {
                                                            Some(true) => {
                                                                ui.colored_label(GREEN, "✓ 正解");
                                                            }
                                                            Some(false) => {
                                                                ui.colored_label(
                                                                    RED,
                                                                    "お手本を見て、もう一度",
                                                                );
                                                            }
                                                            None => {
                                                                ui.weak(" ");
                                                            }
                                                        }
                                                    });
                                                });
                                        } else {
                                            ui.label(
                                                egui::RichText::new(value).color(GOLD).size(19.0),
                                            );
                                        }
                                    }
                                    ui.end_row();
                                }
                            });
                    });
                if let Some(index) = submitted {
                    exercise.check(index, self.strict);
                    if exercise.cells[index].correct == Some(true) {
                        self.focus = (1..=exercise.cells.len())
                            .map(|n| (index + n) % exercise.cells.len())
                            .find(|i| exercise.cells[*i].correct != Some(true));
                    } else {
                        self.focus = Some(index);
                    }
                    ui.ctx().request_repaint();
                }
            });
        leave
    }
}

const BG: Color32 = Color32::from_rgb(22, 23, 36);
const CARD: Color32 = Color32::from_rgb(35, 38, 56);
const EDGE: Color32 = Color32::from_rgb(65, 71, 94);
const GOLD: Color32 = Color32::from_rgb(255, 213, 76);
const WHITE: Color32 = Color32::from_rgb(247, 246, 236);
const MUTED: Color32 = Color32::from_rgb(177, 185, 207);
const GREEN: Color32 = Color32::from_rgb(113, 222, 163);
const RED: Color32 = Color32::from_rgb(255, 135, 135);
fn panel_frame() -> egui::Frame {
    egui::Frame::new()
        .fill(BG)
        .inner_margin(12)
        .stroke(egui::Stroke::new(1.0, EDGE))
}
fn apply_theme(ui: &mut egui::Ui) {
    let style = ui.style_mut();
    style.visuals = egui::Visuals::dark();
    style.visuals.override_text_color = Some(WHITE);
    style.visuals.weak_text_color = Some(MUTED);
    style.visuals.panel_fill = BG;
    style.visuals.extreme_bg_color = Color32::from_rgb(15, 17, 27);
    style.visuals.selection.bg_fill = Color32::from_rgb(74, 67, 37);
    style.visuals.selection.stroke = egui::Stroke::new(1.5, GOLD);
    for widget in [
        &mut style.visuals.widgets.inactive,
        &mut style.visuals.widgets.hovered,
        &mut style.visuals.widgets.active,
    ] {
        widget.corner_radius = egui::CornerRadius::same(8);
        widget.bg_fill = CARD;
    }
    style.visuals.widgets.hovered.bg_stroke = egui::Stroke::new(1.5, GOLD);
    style.visuals.widgets.active.bg_stroke = egui::Stroke::new(2.0, GOLD);
    style.spacing.button_padding = egui::vec2(12.0, 7.0);
    style.spacing.item_spacing = egui::vec2(8.0, 6.0);
}
fn title(ui: &mut egui::Ui) {
    let font = egui::FontId::new(34.0, egui::FontFamily::Name("arcade".into()));
    let galley = ui.painter().layout_no_wrap("写経でGO!".into(), font, GOLD);
    let (rect, _) =
        ui.allocate_exact_size(galley.size() + egui::vec2(8.0, 8.0), egui::Sense::hover());
    let pos = rect.min + egui::vec2(4.0, 4.0);
    for (x, y) in [(-2.0, 0.0), (2.0, 0.0), (0.0, -2.0), (0.0, 2.0)] {
        ui.painter().galley_with_override_text_color(
            pos + egui::vec2(x, y),
            galley.clone(),
            Color32::BLACK,
        );
    }
    ui.painter().galley(pos, galley, GOLD);
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn alternatives_macrons_and_missing_forms() {
        let mut noun = Exercise::new(
            vec![
                vec!["格".into(), "複数".into()],
                vec!["与格".into(), "deīs deābus".into()],
                vec!["欠如".into(), "—".into()],
            ],
            true,
        );
        assert_eq!(noun.cells.len(), 1);
        noun.cells[0].input = "deabus".into();
        noun.check(0, false);
        assert_eq!(noun.completed(), 1);
        noun.check(0, true);
        assert_eq!(noun.completed(), 0);
        noun.cells[0].input = "deābus".into();
        noun.check(0, true);
        assert_eq!(noun.completed(), 1);
        let mut verb = Exercise::new(
            vec![
                vec!["時制".into(), "1s".into()],
                vec!["完了".into(), "rēctus sum / rēcta sum / rēctum sum".into()],
            ],
            false,
        );
        for (value, correct) in [("rēcta sum", true), ("rēcta", false), ("", false)] {
            verb.cells[0].input = value.into();
            verb.check(0, true);
            assert_eq!(verb.cells[0].correct, Some(correct));
        }
    }
    #[test]
    fn random_range_masks_only_requested_forms_and_keeps_manual_drafts_separate() {
        let data: serde_json::Value =
            serde_json::from_str(include_str!("../data/swadesh.json")).unwrap();
        let words: Vec<Word> = serde_json::from_value(data["words"].clone()).unwrap();
        let ctx = egui::Context::default();
        crate::configure_ui(&ctx);
        let mut screen = Screen::default();
        screen.range.verbs = false;
        screen.range.declensions = [false, false, true, false, false];
        screen.random_question(&words, 42);
        let word = words
            .iter()
            .find(|w| Some(w.number) == screen.selected)
            .unwrap();
        assert!(word.principal_parts.contains("third declension"));
        let e = screen.random_exercise.as_ref().unwrap();
        assert_eq!(e.cells.len(), 4);
        assert!(e.cells.iter().all(|c| c.input.is_empty()));
        frame(&mut screen, &ctx, &words, vec![]);
        assert!(
            screen.exercises.is_empty(),
            "random table must not poison the manual draft cache"
        );
        screen.range.nouns = false;
        screen.range.verbs = true;
        screen.range.tenses = [false, false, false, true, false, false];
        screen.range.moods = [true, false, false];
        screen.range.voices = [true, false];
        screen.range.blanks = 24;
        screen.random_question(&words, 50);
        let e = screen.random_exercise.as_ref().unwrap();
        assert_eq!(e.table.len(), 4);
        assert!(e.table[0][0].ends_with(" / 完了"));
        assert!(e.table[0][0].starts_with("直説法"));
        assert!(e.cells.len() < 6, "at least one form remains given");
        frame(&mut screen, &ctx, &words, vec![]);
        screen.range.tenses = [false; 6];
        let previous = screen.selected;
        screen.random_question(&words, 2);
        assert_eq!(screen.selected, previous);
        assert!(!screen.error.is_empty());
    }
    fn frame(screen: &mut Screen, ctx: &egui::Context, words: &[Word], events: Vec<egui::Event>) {
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1120.0, 780.0),
            )),
            focused: true,
            events,
            ..Default::default()
        };
        let mut out = ctx.run_ui(input, |ui| {
            screen.draw(ui, words);
        });
        assert!(!out.shapes.is_empty());
        out.textures_delta.clear();
    }
    #[test]
    fn keyboard_completion_and_drafts_survive_switching() {
        let path = std::env::temp_dir().join(format!("latin-copy-{}.sqlite3", std::process::id()));
        let db = latin_vocab::Database::open(&path).unwrap();
        let words = db.words().unwrap();
        let noun = words.iter().find(|w| w.pos == "Noun").unwrap();
        let verb = words.iter().find(|w| w.pos == "Verb").unwrap();
        let mut screen = Screen::default();
        screen.select(noun);
        let ctx = egui::Context::default();
        crate::configure_ui(&ctx);
        frame(&mut screen, &ctx, &words, vec![]);
        let key = (noun.number, 0);
        assert_eq!(screen.exercises[&key].cells.len(), 12);
        screen.focus = Some(0);
        frame(&mut screen, &ctx, &words, vec![]);
        let id = egui::Id::new(("copy_input", key, 0usize));
        assert!(ctx.memory(|m| m.has_focus(id)));
        let answer = screen.exercises[&key].cells[0].model.clone();
        frame(
            &mut screen,
            &ctx,
            &words,
            vec![egui::Event::Text(answer.clone())],
        );
        frame(
            &mut screen,
            &ctx,
            &words,
            vec![egui::Event::Key {
                key: Key::Enter,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            }],
        );
        assert_eq!(screen.exercises[&key].completed(), 1);
        frame(&mut screen, &ctx, &words, vec![]);
        assert!(ctx.memory(|m| m.has_focus(egui::Id::new(("copy_input", key, 1usize)))));
        screen.select(verb);
        frame(&mut screen, &ctx, &words, vec![]);
        assert!(!screen.exercises[&(verb.number, 0)].cells.is_empty());
        screen.change_table(1);
        frame(&mut screen, &ctx, &words, vec![]);
        screen.select(noun);
        frame(&mut screen, &ctx, &words, vec![]);
        assert_eq!(screen.exercises[&key].cells[0].input, answer);
        assert_eq!(screen.exercises[&key].completed(), 1);
        assert!(
            db.words()
                .unwrap()
                .iter()
                .filter(|w| w.pos == "Verb")
                .all(|w| w.tables.is_empty())
        );
        drop(db);
        std::fs::remove_file(path).unwrap();
    }
}
