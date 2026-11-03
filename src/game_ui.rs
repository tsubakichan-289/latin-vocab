use eframe::egui::{
    self, Color32 as Color, FontFamily, FontId, Pos2, Rect, Sense, Stroke, StrokeKind, Vec2,
};
use latin_vocab::{
    Database, Word,
    game::{self, Config, Difficulty, Direction, Outcome, Phase, Run},
};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const WHITE: Color = Color::from_rgb(255, 253, 244);
const GOLD: Color = Color::from_rgb(255, 214, 70);
const INK: Color = Color::from_rgb(13, 12, 20);
const RED: Color = Color::from_rgb(244, 46, 59);
const GREEN: Color = Color::from_rgb(151, 211, 0);
fn accent(d: Difficulty) -> Color {
    match d {
        Difficulty::Normal => GREEN,
        Difficulty::Hard => Color::from_rgb(248, 146, 0),
        Difficulty::Extreme => RED,
    }
}
fn font(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("arcade".into()))
}

pub enum Action {
    None,
    Library,
    Review(u32),
}
pub struct Screen {
    pub music: crate::music::Music,
    pub config: Config,
    pub run: Option<Run>,
    answer: String,
    settings: bool,
    composing: bool,
    focus_input: bool,
    last_tick: Instant,
    feedback_elapsed: f32,
    record_attempted: bool,
    best: u32,
    message: String,
}
impl Default for Screen {
    fn default() -> Self {
        Self {
            music: crate::music::Music::default(),
            config: Config::default(),
            run: None,
            answer: String::new(),
            settings: false,
            composing: false,
            focus_input: true,
            last_tick: Instant::now(),
            feedback_elapsed: 0.0,
            record_attempted: false,
            best: 0,
            message: String::new(),
        }
    }
}
impl Screen {
    pub fn enter(&mut self) {
        self.last_tick = Instant::now();
    }
    pub fn leave(&mut self) {
        if let Some(run) = &mut self.run
            && run.phase != Phase::Finished
        {
            run.paused = true;
        }
    }
    pub fn start(&mut self, words: &[Word], db: &Database) {
        let pool = match game::question_pool(words, &self.config) {
            Ok(p) => p,
            Err(e) => {
                self.message = format!("問題を作成できません: {e}");
                return;
            }
        };
        let seed = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64;
        self.run = Run::start(pool, self.config.clone(), seed);
        if self.run.is_none() {
            self.message = "出題できる単語がありません。出題・設定を変更してください。".into();
            return;
        }
        self.music.round_started();
        self.answer.clear();
        self.message.clear();
        self.composing = false;
        self.focus_input = true;
        self.record_attempted = false;
        self.feedback_elapsed = 0.0;
        match db.game_best(&self.config) {
            Ok(best) => self.best = best,
            Err(e) => self.message = format!("記録を読み込めません: {e}"),
        }
        self.enter();
    }
    fn next(&mut self) {
        if let Some(run) = &mut self.run {
            run.advance();
        }
        self.answer.clear();
        self.composing = false;
        self.focus_input = true;
        self.feedback_elapsed = 0.0;
    }
    pub fn draw(&mut self, ui: &mut egui::Ui, words: &[Word], db: &Database) -> Action {
        let now = Instant::now();
        let dt = now.duration_since(self.last_tick).as_secs_f32();
        self.last_tick = now;
        let focused = ui.input(|i| i.focused);
        if let Some(run) = &mut self.run {
            if !focused && run.phase != Phase::Finished {
                run.paused = true;
            }
            let previous = run.phase;
            run.tick(dt);
            if previous == Phase::Playing && run.phase == Phase::Feedback {
                self.feedback_elapsed = 0.0;
            }
            if run.phase == Phase::Feedback && !run.paused && previous == Phase::Feedback {
                self.feedback_elapsed += dt;
                if run
                    .attempts
                    .last()
                    .is_some_and(|a| a.outcome == Outcome::Correct)
                    && self.feedback_elapsed >= 0.8
                {
                    self.next();
                }
            }
        }
        let bounds = ui.max_rect();
        let scale = (bounds.width() / 1280.0).min(bounds.height() / 720.0);
        let c = Canvas {
            rect: Rect::from_center_size(bounds.center(), Vec2::new(1280.0, 720.0) * scale),
            s: scale,
        };
        ui.painter().rect_filled(bounds, 0.0, INK);
        let mut action = Action::None;
        ui.scope_builder(egui::UiBuilder::new().max_rect(c.rect), |ui| {
            ui.set_clip_rect(c.rect);
            ui.visuals_mut().weak_text_color = Some(Color::from_gray(155));
            if self.run.is_none() {
                action = self.menu(ui, &c, words, db);
            } else if self
                .run
                .as_ref()
                .is_some_and(|r| r.phase == Phase::Finished)
            {
                action = self.results(ui, &c, words, db);
            } else {
                action = self.play(ui, &c);
            }
            c.text(
                ui,
                640.0,
                712.0,
                "Tab / Shift+Tab 選択　Enter 決定　← → 調整　Esc 戻る・一時停止",
                12.0,
                WHITE,
                1200.0,
            );
            if !self.message.is_empty() {
                c.text(ui, 640.0, 630.0, &self.message, 18.0, WHITE, 1120.0);
            }
        });
        self.music.observe(self.run.as_ref());
        if !matches!(action, Action::None) {
            self.leave();
        }
        action
    }
    fn menu(&mut self, ui: &mut egui::Ui, c: &Canvas, words: &[Word], db: &Database) -> Action {
        c.checker(ui);
        c.logo(ui);
        c.text(
            ui,
            980.0,
            45.0,
            self.config.direction.label(),
            25.0,
            WHITE,
            500.0,
        );
        c.text(
            ui,
            980.0,
            80.0,
            "16問に挑戦！  ライフは3つ",
            20.0,
            GOLD,
            500.0,
        );
        if !self.settings {
            let left = ui.input(|i| i.key_pressed(egui::Key::ArrowLeft));
            let right = ui.input(|i| i.key_pressed(egui::Key::ArrowRight));
            if left || right {
                let i = Difficulty::ALL
                    .iter()
                    .position(|d| *d == self.config.difficulty)
                    .unwrap();
                self.config.difficulty = Difficulty::ALL[(i + if right { 1 } else { 2 }) % 3];
            }
        }
        for (i, d) in Difficulty::ALL.into_iter().enumerate() {
            let selected = d == self.config.difficulty;
            let x = 108.0 + i as f32 * 366.0;
            let rect = c.r(x, 142.0, 332.0, 448.0);
            let response = ui.interact(
                rect,
                ui.id().with(("difficulty", i)),
                if self.settings {
                    Sense::hover()
                } else {
                    Sense::click()
                },
            );
            if response.clicked() && !self.settings {
                self.config.difficulty = d;
            }
            let color = accent(d);
            ui.painter().rect_filled(rect, 0.0, color);
            // Oversized translucent letterforms give each panel the layered poster look.
            c.text(
                ui,
                x + 160.0,
                345.0,
                ["A", "V", "Ω"][i],
                285.0,
                Color::from_black_alpha(35),
                340.0,
            );
            for band in 0..4 {
                let y = 200.0 + band as f32 * 100.0;
                ui.painter().line_segment(
                    [c.p(x, y + 50.0), c.p(x + 332.0, y - 10.0)],
                    Stroke::new(24.0 * c.s, Color::from_black_alpha(14)),
                );
            }
            ui.painter()
                .rect_filled(c.r(x, 142.0, 332.0, 72.0), 0.0, Color::from_white_alpha(95));
            c.text(ui, x + 166.0, 177.0, d.label(), 43.0, WHITE, 312.0);
            let examples = match (self.config.direction, i) {
                (Direction::JapaneseToLatin, 0) => ["水", "月", "名前"],
                (Direction::JapaneseToLatin, 1) => ["聞く", "心臓", "投げる"],
                (Direction::JapaneseToLatin, _) => ["呼吸する", "膨らむ", "においを嗅ぐ"],
                (Direction::LatinToJapanese, 0) => ["aqua", "lūna", "nōmen"],
                (Direction::LatinToJapanese, 1) => ["audiō", "cor", "iaciō"],
                (Direction::LatinToJapanese, _) => ["respīrō", "augeō", "olfaciō"],
                (Direction::Inflection, 0) => ["aqua → aquae", "主格・属格", "現在形"],
                (Direction::Inflection, 1) => ["rēxī", "受動態", "未来形"],
                (Direction::Inflection, _) => ["接続法", "過去完了", "rēctus sum"],
            };
            for (j, example) in examples.into_iter().enumerate() {
                c.text(
                    ui,
                    x + 166.0 + if j % 2 == 0 { 24.0 } else { -24.0 },
                    275.0 + j as f32 * 82.0,
                    example,
                    44.0,
                    Color::from_rgb(255, 229, 132),
                    285.0,
                );
            }
            ui.painter().rect_filled(
                c.r(x, 490.0, 332.0, 100.0),
                0.0,
                Color::from_black_alpha(125),
            );
            let (line1, line2) = match d {
                Difficulty::Normal => ("はじめての方におすすめ", "1問20秒からスタート"),
                Difficulty::Hard => ("覚えた単語を、すばやく！", "1問12秒からスタート"),
                Difficulty::Extreme => ("迷っている時間はない！", "1問8秒からスタート"),
            };
            c.text(ui, x + 166.0, 518.0, line1, 23.0, WHITE, 314.0);
            c.text(ui, x + 166.0, 555.0, line2, 23.0, WHITE, 314.0);
            ui.painter().rect_stroke(
                rect.expand(if selected { 8.0 * c.s } else { 0.0 }),
                0.0,
                Stroke::new(
                    if selected || response.has_focus() {
                        9.0 * c.s
                    } else {
                        2.0 * c.s
                    },
                    if selected || response.has_focus() {
                        GOLD
                    } else {
                        Color::from_black_alpha(60)
                    },
                ),
                StrokeKind::Outside,
            );
            if selected {
                c.text(ui, x + 166.0, 116.0, "▼", 26.0, GOLD, 150.0);
            }
        }
        if self.settings {
            self.settings(ui, c);
            return Action::None;
        }
        let exit = c.button(
            ui,
            "menu_exit",
            c.r(20.0, 650.0, 360.0, 52.0),
            "Esc   単語帳へ",
            INK,
            26.0,
        );
        if c.button(
            ui,
            "settings",
            c.r(450.0, 650.0, 360.0, 52.0),
            "↓   出題・設定",
            INK,
            26.0,
        ) || ui.input(|i| i.key_pressed(egui::Key::ArrowDown))
        {
            self.settings = true;
            if let Some(id) = ui.memory(|m| m.focused()) {
                ui.memory_mut(|m| m.surrender_focus(id));
            }
        }
        if c.button(
            ui,
            "start",
            c.r(860.0, 650.0, 400.0, 52.0),
            "Space   はじめる！",
            INK,
            30.0,
        ) || default_key(ui, egui::Key::Space)
        {
            self.start(words, db);
        }
        if exit || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            Action::Library
        } else {
            Action::None
        }
    }
    fn settings(&mut self, ui: &mut egui::Ui, c: &Canvas) {
        ui.painter()
            .rect_filled(c.rect, 0.0, Color::from_black_alpha(210));
        ui.painter().rect(
            c.r(235.0, 105.0, 810.0, 595.0),
            12.0,
            INK,
            Stroke::new(4.0 * c.s, WHITE),
            StrokeKind::Inside,
        );
        c.text(ui, 640.0, 165.0, "出題・設定", 36.0, GOLD, 600.0);
        for (i, direction) in Direction::ALL.into_iter().enumerate() {
            let color = if self.config.direction == direction {
                Color::from_rgb(48, 111, 177)
            } else {
                Color::from_rgb(47, 43, 59)
            };
            if c.button(
                ui,
                (&"direction", i),
                c.r(290.0, 210.0 + i as f32 * 68.0, 700.0, 54.0),
                direction.label(),
                color,
                26.0,
            ) {
                self.config.direction = direction;
            }
        }
        ui.scope_builder(
            egui::UiBuilder::new().max_rect(c.r(320.0, 430.0, 660.0, 88.0)),
            |ui| {
                ui.visuals_mut().override_text_color = Some(WHITE);
                ui.style_mut().override_font_id = Some(font(20.0 * c.s));
                ui.checkbox(
                    &mut self.config.strict_macrons,
                    "ラテン語の長音記号も厳密に判定",
                );
                ui.checkbox(
                    &mut self.config.unlearned_only,
                    "単語帳で未習得の単語だけ出題",
                );
            },
        );
        c.text(
            ui,
            640.0,
            515.0,
            "日本語は漢字・ひらがな・カタカナで入力できます",
            18.0,
            WHITE,
            730.0,
        );
        ui.scope_builder(
            egui::UiBuilder::new().max_rect(c.r(320.0, 547.0, 660.0, 76.0)),
            |ui| {
                ui.visuals_mut().override_text_color = Some(WHITE);
                ui.style_mut().override_font_id = Some(font(20.0 * c.s));
                ui.horizontal(|ui| {
                    ui.checkbox(&mut self.music.enabled, "BGM");
                    ui.add(egui::Slider::new(&mut self.music.volume, 0.0..=1.0).text("音量"));
                });
                ui.horizontal(|ui| {
                    ui.checkbox(&mut self.music.se_enabled, "SE");
                    ui.add(egui::Slider::new(&mut self.music.se_volume, 0.0..=1.0).text("音量"));
                });
                if let Some(error) = self.music.error.clone() {
                    ui.horizontal_wrapped(|ui| {
                        ui.label(egui::RichText::new(error).size(13.0 * c.s));
                        if ui.button("再試行").clicked() {
                            self.music.error = None;
                        }
                    });
                }
            },
        );
        if c.button(
            ui,
            "close_settings",
            c.r(420.0, 636.0, 440.0, 46.0),
            "設定を決定",
            Color::from_rgb(42, 108, 78),
            24.0,
        ) || ui.input(|i| i.key_pressed(egui::Key::Escape))
        {
            self.settings = false;
        }
    }
    fn play(&mut self, ui: &mut egui::Ui, c: &Canvas) -> Action {
        ui.ctx().request_repaint_after(Duration::from_millis(16));
        let run = self.run.as_ref().unwrap();
        let index = if run.phase == Phase::Feedback {
            run.attempts.last().unwrap().original_index
        } else {
            run.current_index()
        };
        let question = if run.phase == Phase::Feedback {
            &run.attempts.last().unwrap().question
        } else {
            run.current().unwrap()
        };
        c.stage(ui, index / 4);
        for i in 0..run.questions.len() {
            let x = 30.0 + i as f32 * 59.0;
            let rect = c.r(x, 20.0, 53.0, 47.0);
            let color = if i != index && !run.attempts.iter().any(|a| a.original_index == i) {
                Color::from_rgb(40, 37, 49)
            } else {
                [
                    Color::from_rgb(24, 133, 225),
                    GREEN,
                    Color::from_rgb(246, 164, 20),
                    RED,
                ][i / 4]
            };
            ui.painter().rect(
                rect,
                0.0,
                color,
                Stroke::new(
                    if i == index { 4.0 * c.s } else { 1.0 * c.s },
                    if i == index {
                        WHITE
                    } else {
                        Color::from_gray(160)
                    },
                ),
                StrokeKind::Inside,
            );
            c.text(ui, x + 26.0, 43.0, &format!("{}", i + 1), 24.0, WHITE, 50.0);
            if run
                .attempts
                .iter()
                .rev()
                .find(|a| a.original_index == i)
                .is_some_and(|a| a.outcome != Outcome::Correct)
            {
                ui.painter().line_segment(
                    [rect.left_top(), rect.right_bottom()],
                    Stroke::new(4.0 * c.s, RED),
                );
                ui.painter().line_segment(
                    [rect.right_top(), rect.left_bottom()],
                    Stroke::new(4.0 * c.s, RED),
                );
            }
        }
        c.text(
            ui,
            1110.0,
            42.0,
            run.config.difficulty.label(),
            28.0,
            accent(run.config.difficulty),
            250.0,
        );
        c.text(
            ui,
            640.0,
            119.0,
            &if (run.phase == Phase::Feedback && run.attempts.last().is_some_and(|a| a.is_review))
                || (run.phase == Phase::Playing && run.is_review())
            {
                format!("復習 第{}問  /  正解で LIFE +1", index + 1)
            } else {
                format!("LEVEL {}    /    第{}問", index / 4 + 1, index + 1)
            },
            30.0,
            WHITE,
            850.0,
        );
        c.text(
            ui,
            1120.0,
            100.0,
            &format!("SCORE  {:05}", run.score),
            22.0,
            GOLD,
            280.0,
        );
        let limit = run.config.difficulty.seconds(index / 4);
        ui.painter().rect_filled(
            c.r(115.0, 160.0, 1050.0, 13.0),
            2.0,
            Color::from_black_alpha(190),
        );
        ui.painter().rect_filled(
            c.r(
                115.0,
                160.0,
                1050.0 * (run.remaining / limit).clamp(0.0, 1.0),
                13.0,
            ),
            2.0,
            if run.remaining < 5.0 { RED } else { GOLD },
        );
        c.text(
            ui,
            88.0,
            127.0,
            &format!("{:02}", run.remaining.ceil() as u32),
            52.0,
            if run.remaining < 5.0 { RED } else { WHITE },
            100.0,
        );
        c.text(ui, 640.0, 227.0, &question.clue, 27.0, WHITE, 1130.0);
        let size = if question.prompt.chars().count() > 10 {
            88.0
        } else {
            116.0
        };
        c.text(ui, 640.0, 340.0, &question.prompt, size, GOLD, 1100.0);
        for i in 0..3 {
            c.heart(ui, 64.0 + i as f32 * 52.0, 600.0, i < run.lives);
        }
        c.text(
            ui,
            970.0,
            614.0,
            &format!("{} COMBO", run.combo),
            26.0,
            GOLD,
            330.0,
        );
        let paused = run.paused;
        if paused {
            ui.painter()
                .rect_filled(c.rect, 0.0, Color::from_black_alpha(225));
            c.text(ui, 640.0, 252.0, "PAUSE", 88.0, GOLD, 800.0);
            c.text(
                ui,
                640.0,
                325.0,
                "ひと息ついたら、もう一度！",
                28.0,
                WHITE,
                850.0,
            );
            if c.button(
                ui,
                "resume",
                c.r(400.0, 395.0, 480.0, 65.0),
                "Enter   つづける",
                Color::from_rgb(40, 108, 90),
                30.0,
            ) || default_key(ui, egui::Key::Enter)
                || ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape))
            {
                self.run.as_mut().unwrap().paused = false;
                self.focus_input = true;
                self.enter();
            }
            if c.button(
                ui,
                "give_up",
                c.r(400.0, 480.0, 480.0, 58.0),
                "諦める",
                Color::from_rgb(170, 41, 48),
                28.0,
            ) {
                self.run = None;
                self.answer.clear();
                self.composing = false;
                self.settings = false;
                self.message.clear();
                return Action::None;
            }
            if c.button(
                ui,
                "pause_exit",
                c.r(400.0, 560.0, 480.0, 58.0),
                "単語帳へ戻る",
                INK,
                26.0,
            ) {
                return Action::Library;
            }
            return Action::None;
        }
        if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.run.as_mut().unwrap().paused = true;
            return Action::None;
        }
        if run.phase == Phase::Playing
            && ui.input(|i| {
                i.events.iter().any(|event| {
                    matches!(event,
                egui::Event::Key { key: egui::Key::F2, pressed: true, repeat: false, modifiers, .. }
                    if *modifiers == egui::Modifiers::NONE)
                })
            })
            && ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::F2))
        {
            self.run.as_mut().unwrap().skip();
            self.feedback_elapsed = 0.0;
            return Action::None;
        }
        if run.phase == Phase::Playing {
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
            let enter = !commit && !self.composing && default_key(ui, egui::Key::Enter);
            let rect = c.r(175.0, 490.0, 860.0, 77.0);
            ui.painter().rect(
                rect,
                5.0,
                Color::from_black_alpha(235),
                Stroke::new(3.0 * c.s, WHITE),
                StrokeKind::Inside,
            );
            let response = ui.put(
                rect.shrink(7.0 * c.s),
                egui::TextEdit::singleline(&mut self.answer)
                    .id(egui::Id::new("game_answer"))
                    .font(font(36.0 * c.s))
                    .text_color(WHITE)
                    .frame(egui::Frame::NONE)
                    .vertical_align(egui::Align::Center)
                    .hint_text(
                        egui::RichText::new(if question.japanese {
                            "日本語で入力"
                        } else {
                            "ラテン語で入力"
                        })
                        .font(font(30.0 * c.s)),
                    ),
            );
            if self.focus_input {
                response.request_focus();
                self.focus_input = false;
            }
            if c.button(
                ui,
                "answer",
                c.r(1060.0, 490.0, 170.0, 77.0),
                "GO!",
                Color::from_rgb(186, 31, 48),
                43.0,
            ) || enter
            {
                self.run.as_mut().unwrap().submit(&self.answer);
                self.feedback_elapsed = 0.0;
            }
            c.text(
                ui,
                640.0,
                441.0,
                if self.config.strict_macrons {
                    "長音記号も入力しよう"
                } else {
                    "長音記号なしでもOK！"
                },
                19.0,
                WHITE,
                900.0,
            );
        } else {
            let attempt = run.attempts.last().unwrap();
            let (label, color) = match attempt.outcome {
                Outcome::Correct if attempt.life_recovered => ("正解！ LIFE +1", GREEN),
                Outcome::Correct => ("正解！", GREEN),
                Outcome::Wrong => ("不正解", RED),
                Outcome::Timeout => ("TIME UP!", RED),
                Outcome::Skipped => ("パス", RED),
            };
            c.text(ui, 640.0, 428.0, label, 54.0, color, 800.0);
            c.text(
                ui,
                640.0,
                493.0,
                &format!("答え  {}", attempt.question.solution()),
                30.0,
                WHITE,
                1120.0,
            );
            if attempt.outcome == Outcome::Correct {
                c.text(
                    ui,
                    640.0,
                    552.0,
                    &format!("+{} POINTS", attempt.points),
                    30.0,
                    GOLD,
                    700.0,
                );
            } else if c.button(
                ui,
                "next",
                c.r(395.0, 540.0, 490.0, 54.0),
                "Enter   次へ",
                INK,
                28.0,
            ) || default_key(ui, egui::Key::Enter)
            {
                self.next();
            }
        }
        if c.button(
            ui,
            "pause",
            c.r(25.0, 650.0, 330.0, 50.0),
            "Esc   一時停止",
            INK,
            24.0,
        ) {
            self.run.as_mut().unwrap().paused = true;
        }
        c.text(ui, 640.0, 675.0, "Enter   解答する", 23.0, WHITE, 420.0);
        if c.button(
            ui,
            "skip",
            c.r(960.0, 650.0, 290.0, 50.0),
            "F2  パス −1 LIFE",
            INK,
            23.0,
        ) {
            self.run.as_mut().unwrap().skip();
            self.feedback_elapsed = 0.0;
        }
        Action::None
    }
    fn results(&mut self, ui: &mut egui::Ui, c: &Canvas, words: &[Word], db: &Database) -> Action {
        c.checker(ui);
        let run = self.run.as_ref().unwrap();
        if !self.record_attempted {
            self.record_attempted = true;
            match db.record_game(run) {
                Ok(best) => {
                    self.best = best;
                    self.message.clear();
                }
                Err(e) => self.message = format!("記録の保存に失敗しました: {e}"),
            }
        }
        c.text(
            ui,
            640.0,
            78.0,
            if run.cleared() {
                "STAGE CLEAR!"
            } else {
                "GAME OVER"
            },
            72.0,
            if run.cleared() { GOLD } else { RED },
            1160.0,
        );
        c.text(
            ui,
            640.0,
            145.0,
            &format!(
                "{}  /  {}",
                run.config.direction.label(),
                run.config.difficulty.label()
            ),
            24.0,
            WHITE,
            1150.0,
        );
        ui.painter().rect(
            c.r(155.0, 185.0, 970.0, 140.0),
            6.0,
            Color::from_black_alpha(195),
            Stroke::new(3.0 * c.s, GOLD),
            StrokeKind::Inside,
        );
        c.text(ui, 385.0, 222.0, "SCORE", 24.0, WHITE, 400.0);
        c.text(
            ui,
            385.0,
            277.0,
            &format!("{:05}", run.score),
            62.0,
            GOLD,
            450.0,
        );
        c.text(
            ui,
            885.0,
            225.0,
            &format!(
                "正解 {} / {}  復習成功 {}",
                run.attempts
                    .iter()
                    .filter(|a| !a.is_review && a.outcome == Outcome::Correct)
                    .count(),
                run.questions.len(),
                run.attempts
                    .iter()
                    .filter(|a| a.is_review && a.outcome == Outcome::Correct)
                    .count()
            ),
            28.0,
            WHITE,
            420.0,
        );
        c.text(
            ui,
            885.0,
            277.0,
            &format!("最大 {} COMBO", run.best_combo),
            28.0,
            GOLD,
            420.0,
        );
        c.text(
            ui,
            640.0,
            355.0,
            &format!("このルールでの自己ベスト  {:05}", self.best),
            24.0,
            WHITE,
            1100.0,
        );
        let misses = run
            .attempts
            .iter()
            .filter(|a| a.outcome != Outcome::Correct)
            .collect::<Vec<_>>();
        c.text(
            ui,
            640.0,
            400.0,
            if misses.is_empty() {
                "PERFECT!  お見事！"
            } else {
                "間違えた問題を単語帳で確認しよう"
            },
            28.0,
            if misses.is_empty() { GOLD } else { WHITE },
            1150.0,
        );
        for (i, a) in misses.iter().enumerate() {
            let label = format!("{}  →  {}", a.question.prompt, a.question.solution());
            if c.button(
                ui,
                ("review", i),
                c.r(135.0, 435.0 + i as f32 * 53.0, 1010.0, 43.0),
                &label,
                Color::from_rgb(57, 51, 70),
                22.0,
            ) {
                return Action::Review(a.question.word_number);
            }
        }
        if !self.message.is_empty()
            && c.button(
                ui,
                "retry_save",
                c.r(420.0, 590.0, 440.0, 35.0),
                "記録の保存を再試行",
                INK,
                18.0,
            )
        {
            self.record_attempted = false;
        }
        if c.button(
            ui,
            "result_exit",
            c.r(20.0, 650.0, 360.0, 52.0),
            "単語帳へ戻る",
            INK,
            26.0,
        ) {
            return Action::Library;
        }
        if c.button(
            ui,
            "result_menu",
            c.r(450.0, 650.0, 360.0, 52.0),
            "Esc  設定にもどる",
            INK,
            26.0,
        ) || ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape))
        {
            self.run = None;
        }
        if c.button(
            ui,
            "replay",
            c.r(860.0, 650.0, 400.0, 52.0),
            "もう一度！",
            Color::from_rgb(170, 41, 48),
            30.0,
        ) {
            self.start(words, db);
        }
        Action::None
    }
}

struct Canvas {
    rect: Rect,
    s: f32,
}
impl Canvas {
    fn p(&self, x: f32, y: f32) -> Pos2 {
        self.rect.min + Vec2::new(x, y) * self.s
    }
    fn r(&self, x: f32, y: f32, w: f32, h: f32) -> Rect {
        Rect::from_min_size(self.p(x, y), Vec2::new(w, h) * self.s)
    }
    #[allow(clippy::too_many_arguments)] // Virtual-canvas position, typography and width.
    fn text(
        &self,
        ui: &egui::Ui,
        x: f32,
        y: f32,
        text: &str,
        size: f32,
        color: Color,
        max_width: f32,
    ) {
        let painter = ui.painter();
        let mut galley = painter.layout_no_wrap(text.into(), font(size * self.s), color);
        if galley.size().x > max_width * self.s {
            galley = painter.layout_no_wrap(
                text.into(),
                font(size * self.s * (max_width * self.s / galley.size().x)),
                color,
            );
        }
        let pos = self.p(x, y) - galley.size() * 0.5;
        let stroke = (size / 15.0).clamp(1.5, 6.0) * self.s;
        for (dx, dy) in [
            (-1.0, 0.0),
            (1.0, 0.0),
            (0.0, -1.0),
            (0.0, 1.0),
            (-0.7, -0.7),
            (0.7, -0.7),
            (-0.7, 0.7),
            (0.7, 0.7),
        ] {
            painter.galley_with_override_text_color(
                pos + Vec2::new(dx, dy) * stroke,
                galley.clone(),
                INK,
            );
        }
        painter.galley(pos, galley, color);
    }
    fn checker(&self, ui: &egui::Ui) {
        for row in 0..9 {
            for col in 0..16 {
                ui.painter().rect_filled(
                    self.r(col as f32 * 80.0, row as f32 * 80.0, 80.0, 80.0),
                    0.0,
                    if (row + col) % 2 == 0 {
                        Color::from_rgb(45, 42, 56)
                    } else {
                        Color::from_rgb(64, 60, 76)
                    },
                );
            }
        }
    }
    fn logo(&self, ui: &egui::Ui) {
        self.text(ui, 212.0, 49.0, "ラテン語で", 50.0, GOLD, 400.0);
        self.text(ui, 480.0, 47.0, "GO!", 76.0, RED, 220.0);
        self.text(
            ui,
            196.0,
            97.0,
            "LATINA  /  TYPING CHALLENGE",
            15.0,
            WHITE,
            380.0,
        );
    }
    fn button(
        &self,
        ui: &mut egui::Ui,
        id: impl std::hash::Hash + std::fmt::Debug,
        rect: Rect,
        text: &str,
        color: Color,
        size: f32,
    ) -> bool {
        let response = ui.interact(rect, ui.id().with(id), Sense::click());
        ui.painter().rect(
            rect,
            egui::CornerRadius::same((rect.height() * 0.48).min(255.0) as u8),
            if response.hovered() || response.has_focus() {
                color.gamma_multiply(1.35)
            } else {
                color
            },
            Stroke::new(
                2.5 * self.s,
                if response.hovered() || response.has_focus() {
                    GOLD
                } else {
                    WHITE
                },
            ),
            StrokeKind::Inside,
        );
        self.text(
            ui,
            (rect.center().x - self.rect.left()) / self.s,
            (rect.center().y - self.rect.top()) / self.s,
            text,
            size,
            WHITE,
            rect.width() / self.s - 20.0,
        );
        response
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .clicked()
    }
    fn heart(&self, ui: &egui::Ui, x: f32, y: f32, filled: bool) {
        let color = if filled {
            RED
        } else {
            Color::from_rgb(63, 50, 65)
        };
        ui.painter()
            .circle_filled(self.p(x - 8.0, y - 5.0), 12.0 * self.s, color);
        ui.painter()
            .circle_filled(self.p(x + 8.0, y - 5.0), 12.0 * self.s, color);
        ui.painter().add(egui::Shape::convex_polygon(
            vec![
                self.p(x - 20.0, y - 2.0),
                self.p(x + 20.0, y - 2.0),
                self.p(x, y + 22.0),
            ],
            color,
            Stroke::NONE,
        ));
    }
    fn stage(&self, ui: &egui::Ui, stage: usize) {
        let top = [
            Color::from_rgb(33, 44, 73),
            Color::from_rgb(30, 64, 60),
            Color::from_rgb(77, 43, 57),
            Color::from_rgb(48, 25, 67),
        ][stage.min(3)];
        ui.painter().rect_filled(self.rect, 0.0, top);
        // A receding colonnade: original vector scenery keeps the arcade's depth.
        ui.painter().add(egui::Shape::convex_polygon(
            vec![
                self.p(0.0, 720.0),
                self.p(500.0, 275.0),
                self.p(780.0, 275.0),
                self.p(1280.0, 720.0),
            ],
            Color::from_rgb(27, 25, 42),
            Stroke::NONE,
        ));
        for i in 0..8 {
            let t = i as f32 / 8.0;
            let y = 280.0 + t * t * 440.0;
            ui.painter().line_segment(
                [self.p(0.0, y), self.p(1280.0, y)],
                Stroke::new(1.0 * self.s, Color::from_white_alpha(18)),
            );
        }
        for side in [-1.0, 1.0] {
            for depth in 0..5 {
                let f = 1.0 - depth as f32 * 0.16;
                let x = 640.0 + side * (170.0 + 370.0 * f);
                let y = 280.0 - 175.0 * f;
                let w = 76.0 * f;
                let h = 370.0 * f;
                let column = Color::from_rgba_unmultiplied(160, 170, 190, (25.0 + 40.0 * f) as u8);
                ui.painter()
                    .rect_filled(self.r(x - w / 2.0, y, w, h), 0.0, column);
                ui.painter().rect_filled(
                    self.r(x - w * 0.8, y - 13.0 * f, w * 1.6, 20.0 * f),
                    1.0,
                    column,
                );
                ui.painter().rect_filled(
                    self.r(x - w * 0.8, y + h - 6.0 * f, w * 1.6, 20.0 * f),
                    1.0,
                    column,
                );
                for flute in 0..4 {
                    let fx = x - w * 0.35 + flute as f32 * w * 0.23;
                    ui.painter().line_segment(
                        [self.p(fx, y + 15.0 * f), self.p(fx, y + h - 12.0 * f)],
                        Stroke::new(3.0 * f * self.s, Color::from_black_alpha(30)),
                    );
                }
                ui.painter().circle_filled(
                    self.p(x, y + h * 0.58),
                    5.0 * f * self.s,
                    GOLD.gamma_multiply(0.5),
                );
            }
        }
        ui.painter().rect_filled(
            self.r(0.0, 0.0, 1280.0, 185.0),
            0.0,
            Color::from_black_alpha(125),
        );
        ui.painter().rect_filled(
            self.r(0.0, 636.0, 1280.0, 84.0),
            0.0,
            Color::from_black_alpha(185),
        );
    }
}

// Default actions must not steal Enter/Space from a Tab-focused control.
fn default_key(ui: &mut egui::Ui, key: egui::Key) -> bool {
    let focus = ui.memory(|m| m.focused());
    (focus.is_none() || focus == Some(egui::Id::new("game_answer")))
        && ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, key))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn key_frame(
        screen: &mut Screen,
        ctx: &egui::Context,
        words: &[Word],
        db: &Database,
        key: Option<egui::Key>,
    ) {
        let mut input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1120.0, 780.0))),
            focused: true,
            ..Default::default()
        };
        if let Some(key) = key {
            input.events.push(egui::Event::Key {
                key,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            });
        }
        let mut output = ctx.run_ui(input, |ui| {
            screen.draw(ui, words, db);
        });
        output.textures_delta.clear();
        if let Some(key) = key {
            let input = egui::RawInput {
                focused: true,
                events: vec![egui::Event::Key {
                    key,
                    physical_key: None,
                    pressed: false,
                    repeat: false,
                    modifiers: egui::Modifiers::NONE,
                }],
                ..Default::default()
            };
            let mut output = ctx.run_ui(input, |ui| {
                screen.draw(ui, words, db);
            });
            output.textures_delta.clear();
        }
    }
    #[test]
    fn keyboard_can_give_up_without_triggering_resume() {
        let path =
            std::env::temp_dir().join(format!("latin-keyboard-{}.sqlite3", std::process::id()));
        let db = Database::open(&path).unwrap();
        let words = db.words().unwrap();
        let ctx = egui::Context::default();
        crate::configure_ui(&ctx);
        let mut screen = Screen::default();
        screen.start(&words, &db);
        screen.run.as_mut().unwrap().paused = true;
        key_frame(&mut screen, &ctx, &words, &db, None);
        key_frame(&mut screen, &ctx, &words, &db, Some(egui::Key::Tab));
        let first = ctx.memory(|m| m.focused());
        assert!(first.is_some());
        key_frame(&mut screen, &ctx, &words, &db, Some(egui::Key::Tab));
        assert_ne!(first, ctx.memory(|m| m.focused()));
        key_frame(&mut screen, &ctx, &words, &db, Some(egui::Key::Enter));
        assert!(
            screen.run.is_none(),
            "Enter should activate the focused give-up button"
        );
        screen.settings = true;
        if let Some(id) = ctx.memory(|m| m.focused()) {
            ctx.memory_mut(|m| m.surrender_focus(id));
        }
        key_frame(&mut screen, &ctx, &words, &db, None);
        for _ in 0..2 {
            key_frame(&mut screen, &ctx, &words, &db, Some(egui::Key::Tab));
        }
        key_frame(&mut screen, &ctx, &words, &db, Some(egui::Key::Enter));
        assert_eq!(screen.config.direction, Direction::LatinToJapanese);
        for _ in 0..2 {
            key_frame(&mut screen, &ctx, &words, &db, Some(egui::Key::Tab));
        }
        key_frame(&mut screen, &ctx, &words, &db, Some(egui::Key::Space));
        assert!(screen.config.strict_macrons);
        for _ in 0..2 {
            key_frame(&mut screen, &ctx, &words, &db, Some(egui::Key::Tab));
        }
        key_frame(&mut screen, &ctx, &words, &db, Some(egui::Key::Space));
        assert!(!screen.music.enabled);
        key_frame(&mut screen, &ctx, &words, &db, Some(egui::Key::Tab));
        let volume = screen.music.volume;
        key_frame(&mut screen, &ctx, &words, &db, Some(egui::Key::ArrowRight));
        assert!(screen.music.volume > volume);
        screen.settings = false;
        screen.start(&words, &db);
        key_frame(&mut screen, &ctx, &words, &db, None);
        assert_eq!(
            ctx.memory(|m| m.focused()),
            Some(egui::Id::new("game_answer"))
        );
        key_frame(&mut screen, &ctx, &words, &db, Some(egui::Key::F2));
        assert_eq!(
            screen.run.as_ref().unwrap().attempts[0].outcome,
            Outcome::Skipped
        );
        assert_eq!(screen.run.as_ref().unwrap().lives, 2);
        key_frame(&mut screen, &ctx, &words, &db, Some(egui::Key::F2));
        assert_eq!(screen.run.as_ref().unwrap().lives, 2);
        screen.next();
        screen.run.as_mut().unwrap().paused = true;
        key_frame(&mut screen, &ctx, &words, &db, Some(egui::Key::F2));
        assert_eq!(screen.run.as_ref().unwrap().lives, 2);
        drop(db);
        std::fs::remove_file(path).unwrap();
    }
    #[test]
    fn renders_every_state_and_records_a_result_once() {
        let path =
            std::env::temp_dir().join(format!("latin-game-ui-test-{}.sqlite3", std::process::id()));
        let db = Database::open(&path).unwrap();
        let words = db.words().unwrap();
        let ctx = egui::Context::default();
        crate::configure_ui(&ctx);
        let mut screen = Screen::default();
        for size in [[1120., 780.], [760., 520.]] {
            for state in 0..8 {
                screen.run = None;
                screen.settings = state == 1;
                if state >= 2 {
                    screen.start(&words, &db);
                    let r = screen.run.as_mut().unwrap();
                    if state == 3 {
                        r.submit("incorrect");
                    }
                    if state == 4 {
                        r.paused = true;
                    }
                    if state == 5 {
                        for _ in 0..3 {
                            r.skip();
                            r.advance();
                        }
                    }
                    if state >= 6 {
                        r.submit("incorrect");
                        r.advance();
                        for _ in 0..2 {
                            let answer = r.current().unwrap().answers[0].clone();
                            r.submit(&answer);
                            r.advance();
                        }
                        assert!(r.is_review());
                        if state == 7 {
                            let answer = r.current().unwrap().answers[0].clone();
                            r.submit(&answer);
                            assert!(r.attempts.last().unwrap().life_recovered);
                        }
                    }
                }
                let input = egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::from(size))),
                    focused: true,
                    ..Default::default()
                };
                for _ in 0..2 {
                    let mut output = ctx.run_ui(input.clone(), |ui| {
                        screen.draw(ui, &words, &db);
                    });
                    assert!(output.shapes.len() > 20);
                    output.textures_delta.clear();
                }
                if state == 5 {
                    assert!(screen.record_attempted);
                }
            }
        }
        drop(db);
        let connection = rusqlite::Connection::open(&path).unwrap();
        let plays: u32 = connection
            .query_row("SELECT sum(plays) FROM game_records", [], |r| r.get(0))
            .unwrap();
        assert_eq!(plays, 2);
        drop(connection);
        std::fs::remove_file(path).unwrap();
    }
}
