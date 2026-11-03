//! Original synthesized menu/battle music and gameplay sound effects.
use latin_vocab::game::{Outcome, Phase, Run};
use rodio::{OutputStream, Sink, Source, buffer::SamplesBuffer};
use std::f32::consts::TAU;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Cue {
    Start,
    Correct,
    Wrong,
    Timeout,
    Skip,
    Clear,
    Defeat,
}
struct Output {
    bgm: Sink,
    effects: Sink,
    _stream: OutputStream,
    tracks: [SamplesBuffer<f32>; 2],
    battle: Option<bool>,
}
pub struct Music {
    pub enabled: bool,
    pub volume: f32,
    pub se_enabled: bool,
    pub se_volume: f32,
    pub error: Option<String>,
    output: Option<Output>,
    pending: Vec<Cue>,
    attempts: usize,
    finished: bool,
}
impl Default for Music {
    fn default() -> Self {
        Self {
            enabled: true,
            volume: 0.2,
            se_enabled: true,
            se_volume: 0.45,
            error: None,
            output: None,
            pending: vec![],
            attempts: 0,
            finished: false,
        }
    }
}
impl Music {
    pub fn round_started(&mut self) {
        self.attempts = 0;
        self.finished = false;
        self.pending.clear();
        self.pending.push(Cue::Start);
    }
    pub fn observe(&mut self, run: Option<&Run>) {
        let Some(run) = run else {
            return;
        };
        for attempt in run.attempts.iter().skip(self.attempts) {
            self.pending.push(match attempt.outcome {
                Outcome::Correct => Cue::Correct,
                Outcome::Wrong => Cue::Wrong,
                Outcome::Timeout => Cue::Timeout,
                Outcome::Skipped => Cue::Skip,
            });
        }
        self.attempts = run.attempts.len();
        if run.phase == Phase::Finished && !self.finished {
            self.pending.push(if run.cleared() {
                Cue::Clear
            } else {
                Cue::Defeat
            });
            self.finished = true;
        }
    }
    pub fn update(&mut self, active: bool, battle: bool) {
        let play = active && self.enabled && self.volume > 0.0;
        let effects = active && self.se_enabled && self.se_volume > 0.0;
        if (play || (effects && !self.pending.is_empty()))
            && self.output.is_none()
            && self.error.is_none()
        {
            match Self::open() {
                Ok(output) => self.output = Some(output),
                Err(e) => self.error = Some(format!("音声を再生できません: {e}")),
            }
        }
        if let Some(output) = &mut self.output {
            output.bgm.set_volume(self.volume.clamp(0.0, 1.0));
            if play {
                if output.battle != Some(battle) {
                    output.bgm.clear();
                    output
                        .bgm
                        .append(output.tracks[usize::from(battle)].clone().repeat_infinite());
                    output.battle = Some(battle);
                }
                output.bgm.play();
            } else {
                output.bgm.pause();
            }
            output.effects.set_volume(self.se_volume.clamp(0.0, 1.0));
            if !effects {
                output.effects.clear();
            } else {
                for cue in &self.pending {
                    // Replace any lingering effect so feedback is immediate, never backlogged.
                    output.effects.clear();
                    output
                        .effects
                        .append(SamplesBuffer::new(1, RATE, effect(*cue)));
                    output.effects.play();
                }
            }
        }
        self.pending.clear();
    }
    fn open() -> Result<Output, Box<dyn std::error::Error>> {
        let (stream, handle) = OutputStream::try_default()?;
        let bgm = Sink::try_new(&handle)?;
        bgm.pause();
        let effects = Sink::try_new(&handle)?;
        Ok(Output {
            bgm,
            effects,
            _stream: stream,
            battle: None,
            tracks: [
                SamplesBuffer::new(1, RATE, theme()),
                SamplesBuffer::new(1, RATE, battle_theme()),
            ],
        })
    }
}
const RATE: u32 = 44_100;
fn frequency(note: i32) -> f32 {
    440.0 * 2.0_f32.powf((note as f32 - 69.0) / 12.0)
}
fn tone(data: &mut [f32], start: f32, duration: f32, note: i32, gain: f32, bright: bool) {
    let offset = (start * RATE as f32).round() as usize;
    let count = (duration * RATE as f32).round() as usize;
    for (i, sample) in data.iter_mut().skip(offset).take(count).enumerate() {
        let t = i as f32 / RATE as f32;
        let envelope = (t / 0.008).min(1.0) * ((duration - t) / 0.04).clamp(0.0, 1.0);
        let phase = TAU * frequency(note) * t;
        // Limited harmonics keep the chip-like lead softer than a raw square wave.
        let wave = if bright {
            phase.sin() + (phase * 3.0).sin() * 0.25 + (phase * 5.0).sin() * 0.1
        } else {
            phase.sin() + (phase * 2.0).sin() * 0.15
        };
        *sample += gain * envelope * wave;
    }
}
fn theme() -> Vec<f32> {
    let beat = 60.0 / 128.0;
    let mut data = vec![0.0; (32.0 * beat * RATE as f32) as usize];
    let chords = [
        [45, 52, 57],
        [41, 48, 53],
        [48, 55, 60],
        [43, 50, 55],
        [45, 52, 57],
        [41, 48, 53],
        [43, 50, 55],
        [40, 47, 56],
    ];
    let melody = [
        [69, 72, 76, 72, 74, 72, 69, 67],
        [65, 69, 72, 69, 76, 72, 69, 65],
        [67, 72, 76, 79, 76, 74, 72, 76],
        [74, 71, 67, 71, 74, 76, 74, 67],
        [76, 81, 79, 76, 74, 72, 69, 72],
        [77, 76, 72, 69, 72, 69, 65, 69],
        [74, 79, 77, 74, 71, 74, 67, 71],
        [76, 74, 71, 68, 71, 68, 64, 68],
    ];
    for bar in 0..8 {
        for step in 0..8 {
            let start = (bar * 4) as f32 * beat + step as f32 * beat / 2.0;
            tone(&mut data, start, beat * 0.43, melody[bar][step], 0.15, true);
            tone(
                &mut data,
                start,
                beat * 0.4,
                chords[bar][step % 3] + 12,
                0.055,
                true,
            );
            if step % 2 == 0 {
                tone(&mut data, start, beat * 0.8, chords[bar][0], 0.2, false);
            }
        }
    }
    data
}
// 168 BPM minor-key battle loop: driving sixteenth-note ostinato and drums.
fn battle_theme() -> Vec<f32> {
    let beat = 60.0 / 168.0;
    let mut data = vec![0.0; (32.0 * beat * RATE as f32) as usize];
    let roots = [45, 45, 41, 43, 45, 41, 43, 40];
    let leads = [
        [76, 76, 79, 81, 83, 81, 79, 76],
        [74, 76, 72, 74, 71, 72, 68, 71],
        [77, 76, 72, 69, 72, 76, 77, 81],
        [79, 77, 74, 71, 74, 77, 79, 83],
        [81, 83, 84, 83, 81, 79, 76, 79],
        [81, 79, 77, 76, 77, 76, 72, 69],
        [79, 77, 74, 71, 74, 77, 83, 79],
        [80, 76, 71, 68, 71, 74, 80, 83],
    ];
    for bar in 0..8 {
        for step in 0..16 {
            let t = (bar * 4) as f32 * beat + step as f32 * beat / 4.0;
            let root = roots[bar];
            tone(
                &mut data,
                t,
                beat * 0.21,
                root + [0, 12, 7, 12][step % 4],
                0.16,
                true,
            );
            tone(
                &mut data,
                t,
                beat * 0.18,
                root + [24, 31, 36, 31][step % 4],
                0.045,
                true,
            );
            if step % 2 == 0 {
                tone(&mut data, t, beat * 0.44, leads[bar][step / 2], 0.16, true);
                drum(&mut data, t, 0.04, 0.06, false);
            }
            if step % 8 == 0 || step == 10 {
                drum(&mut data, t, 0.13, 0.27, true);
            }
            if step % 8 == 4 {
                drum(&mut data, t, 0.1, 0.18, false);
            }
        }
    }
    data
}
fn drum(data: &mut [f32], start: f32, duration: f32, gain: f32, kick: bool) {
    let mut noise = 0x1234abcd_u32;
    for (i, sample) in data
        .iter_mut()
        .skip((start * RATE as f32) as usize)
        .take((duration * RATE as f32) as usize)
        .enumerate()
    {
        let t = i as f32 / RATE as f32;
        noise ^= noise << 13;
        noise ^= noise >> 17;
        noise ^= noise << 5;
        let wave = if kick {
            (TAU * (48.0 * t + 6.0 * (1.0 - (-30.0 * t).exp()))).sin()
        } else {
            noise as f32 / u32::MAX as f32 * 2.0 - 1.0
        };
        let env = (t / 0.002).min(1.0) * (1.0 - t / duration).max(0.0).powi(3);
        *sample += gain * wave * env;
    }
}
fn effect(cue: Cue) -> Vec<f32> {
    let (notes, step, duration): (&[i32], f32, f32) = match cue {
        Cue::Start => (&[57, 64, 69, 76], 0.065, 0.1),
        Cue::Correct => (&[84, 88, 91], 0.045, 0.09),
        Cue::Wrong => (&[48, 43], 0.09, 0.13),
        Cue::Timeout => (&[88, 76, 88, 64], 0.09, 0.075),
        Cue::Skip => (&[76, 72, 67], 0.05, 0.08),
        Cue::Clear => (&[72, 76, 79, 84, 79, 84, 88], 0.12, 0.2),
        Cue::Defeat => (&[64, 60, 57, 52, 45], 0.15, 0.23),
    };
    let mut data =
        vec![0.0; (((notes.len() - 1) as f32 * step + duration + 0.03) * RATE as f32) as usize];
    for (i, note) in notes.iter().enumerate() {
        tone(&mut data, i as f32 * step, duration, *note, 0.28, true);
    }
    data
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn loop_is_audible_bounded_and_has_quiet_seam() {
        let data = theme();
        assert_eq!(data.len(), 661500);
        assert!(data.iter().all(|x| x.is_finite() && x.abs() < 1.0));
        assert!(data.iter().map(|x| x * x).sum::<f32>() / data.len() as f32 > 0.01);
        assert_eq!(data[0], 0.0);
        assert!(data.iter().rev().take(100).all(|x| *x == 0.0));
    }
    #[test]
    fn inactive_or_muted_music_does_not_open_an_audio_device() {
        let mut m = Music::default();
        m.update(false, false);
        assert!(m.output.is_none());
        m.enabled = false;
        m.update(true, false);
        assert!(m.output.is_none());
        m.enabled = true;
        m.volume = 0.0;
        m.update(true, false);
        assert!(m.output.is_none());
        assert!(m.error.is_none());
    }

    #[test]
    fn battle_and_effects_are_bounded_and_end_quietly() {
        let battle = battle_theme();
        assert!(battle.len() < theme().len());
        for data in std::iter::once(battle).chain(
            [
                Cue::Start,
                Cue::Correct,
                Cue::Wrong,
                Cue::Timeout,
                Cue::Skip,
                Cue::Clear,
                Cue::Defeat,
            ]
            .into_iter()
            .map(effect),
        ) {
            assert!(data.iter().all(|s| s.is_finite() && s.abs() < 1.0));
            assert!(data.iter().any(|s| s.abs() > 0.1));
            assert_eq!(data[0], 0.0);
            assert!(data.iter().rev().take(100).all(|s| *s == 0.0));
        }
    }
    #[test]
    fn cues_fire_once_per_transition_and_muted_events_are_discarded() {
        use latin_vocab::game::{Config, Question};
        let q = Question {
            word_number: 1,
            prompt: "水".into(),
            clue: String::new(),
            answers: vec!["aqua".into()],
            japanese: false,
            level: 0,
        };
        let mut run = Run::start(vec![q], Config::default(), 1).unwrap();
        let mut music = Music::default();
        music.round_started();
        music.observe(Some(&run));
        music.observe(Some(&run));
        assert_eq!(music.pending, vec![Cue::Start]);
        music.update(false, true);
        run.submit("aqua");
        music.observe(Some(&run));
        music.observe(Some(&run));
        assert_eq!(music.pending, vec![Cue::Correct]);
        music.update(false, true);
        run.advance();
        music.observe(Some(&run));
        music.observe(Some(&run));
        assert_eq!(music.pending, vec![Cue::Clear]);
        music.enabled = false;
        music.se_enabled = false;
        music.update(true, false);
        assert!(music.pending.is_empty());
        assert!(music.output.is_none());
        music.round_started();
        assert_eq!(music.pending, vec![Cue::Start]);
    }
}
