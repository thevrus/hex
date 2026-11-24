//! Labs › Play meeting audio: the player under a saved meeting's title, the
//! transcript line that is playing, and Export audio.
//!
//! The player is a play button, the time, a scrubber, and the length. Once
//! a meeting plays, the other lines recede and the spoken one stays in
//! view, and any line plays from its start when pressed. The pane owns one
//! [`Playback`] at a time, made by the first Play, and drops it (stopping
//! the sound and closing the output device) when the meeting, the pane, the
//! window, or the lab goes away.

use std::path::PathBuf;
use std::sync::mpsc::{Receiver, TryRecvError, channel};
use std::time::Duration;

use rune::{Drag, El, Phase, Tx, col, el, row, text};
use rune_kit::{ControlSize, IconButton, IconButtonVariant};

use super::super::controls::{PAUSE, PLAY, Paint};
use super::super::lists::{Tone, error_panel};
use super::super::{Pane, Ui};
use super::{Meeting, Status, Turn, View};
use crate::desktop::i18n::{tr, tr_fill};
use crate::meeting::playback::Playback;

/// How often a playing meeting redraws, so the scrubber glides.
const FRAME: Duration = Duration::from_millis(100);
/// The transcript scroll the highlight keeps the playing line inside.
const TRANSCRIPT: &str = "meeting-transcript";

#[derive(Default)]
pub(in crate::rune_ui) struct Audio {
    playback: Option<Playback>,
    /// Where the scrubber is held while it is dragged, in ms.
    scrub: Option<u64>,
    /// The line last brought into view, so each line scrolls once.
    followed: Option<usize>,
    export: Option<Export>,
    /// A scene's position and state, drawn without a device.
    pub(in crate::rune_ui) preview: Option<(u64, bool)>,
}

/// Export audio for one meeting: running, or how it ended.
struct Export {
    id: String,
    worker: Option<Receiver<Result<PathBuf, String>>>,
    error: Option<String>,
}

/// Whether `meeting`'s audio can play: the lab is on and it is a finished
/// recording whose tracks are still kept.
pub(super) fn offered(ui: &Ui, meeting: &Meeting) -> bool {
    super::available(ui)
        && crate::labs::Feature::MeetingPlayback.on(&ui.settings)
        && crate::labs::Feature::MeetingPlayback.managed().is_none()
        && meeting.audio
        && matches!(meeting.status, Status::Complete | Status::Interrupted)
}

/// The position, length, and state the player draws for `meeting`.
fn state(ui: &Ui, meeting: &Meeting) -> (u64, u64, bool) {
    let audio = &ui.meetings.audio;
    if let Some((position, playing)) = audio.preview {
        return (position, meeting.duration_ms, playing);
    }
    match audio
        .playback
        .as_ref()
        .filter(|playback| playback.id == meeting.id)
    {
        Some(playback) => (
            playback.position_ms(),
            playback.duration_ms().unwrap_or(meeting.duration_ms),
            playback.playing(),
        ),
        None => (0, meeting.duration_ms, false),
    }
}

/// The line playing at `position`: the last that started by then.
fn playing_turn(turns: &[Turn], position: u64) -> Option<usize> {
    turns.iter().rposition(|turn| turn.start_ms <= position)
}

/// The line to highlight: only once this meeting has been played or
/// scrubbed, so an untouched transcript reads as it always has.
pub(super) fn highlighted(ui: &Ui, meeting: &Meeting) -> Option<usize> {
    let audio = &ui.meetings.audio;
    let touched = audio.preview.is_some()
        || audio
            .playback
            .as_ref()
            .is_some_and(|playback| playback.id == meeting.id);
    if !touched {
        return None;
    }
    let (position, _, _) = state(ui, meeting);
    playing_turn(meeting.turns(), audio.scrub.unwrap_or(position))
}

/// The meeting's player, made on its first use.
fn player<'a>(ui: &'a mut Ui, id: &str) -> Option<&'a Playback> {
    let folder = ui.meetings.meeting(id)?.folder.clone();
    let audio = &mut ui.meetings.audio;
    if audio
        .playback
        .as_ref()
        .is_none_or(|playback| playback.id != id)
    {
        let waker = ui.modes.waker.clone();
        audio.followed = None;
        audio.playback = Some(Playback::new(id.to_string(), folder, move || {
            if let Some(waker) = &waker {
                waker();
            }
        }));
    }
    audio.playback.as_ref()
}

fn toggle(ui: &mut Ui, id: &str) {
    if let Some(playback) = player(ui, id) {
        if playback.playing() {
            playback.pause();
        } else {
            playback.play();
        }
    }
}

/// Plays `id` from `ms`: a pressed line, or a released scrubber.
fn play_from(ui: &mut Ui, id: &str, ms: u64, play: bool) {
    if let Some(playback) = player(ui, id) {
        playback.seek(ms);
        if play {
            playback.play();
        }
    }
    ui.meetings.audio.followed = None;
}

/// `1:05:09`, `4:07`, or `0:03`: a player's clock.
fn clock(ms: u64) -> String {
    let seconds = ms / 1_000;
    let (hours, minutes, seconds) = (seconds / 3_600, seconds / 60 % 60, seconds % 60);
    if hours > 0 {
        format!("{hours}:{minutes:02}:{seconds:02}")
    } else {
        format!("{minutes}:{seconds:02}")
    }
}

/// The bar under the title: play, the time, the scrubber, and the length,
/// then why it could not play, if it couldn't.
pub(super) fn bar(ui: &Ui, p: Paint, meeting: &Meeting) -> El {
    let (position, duration, playing) = state(ui, meeting);
    let audio = &ui.meetings.audio;
    let shown = audio.scrub.unwrap_or(position).min(duration);
    let progress = if duration == 0 {
        0.0
    } else {
        shown as f32 / duration as f32
    };
    let id = meeting.id.clone();
    let button = IconButton::new(if playing { &PAUSE } else { &PLAY })
        .variant(IconButtonVariant::Ghost)
        .control_size(ControlSize::Small)
        .label(if playing { tr("Pause") } else { tr("Play") })
        .on_press(move |ui: &mut Ui| toggle(ui, &id))
        .build(p.k)
        .key("meeting-play");
    let time = |key: &str, ms: u64| {
        text(clock(ms))
            .key(key)
            .font(p.k.ty.mono)
            .color(p.faint())
            .a11y_hidden()
    };
    let mut player = col().key("meeting-player").gap(6.0).pt(12.0).child(
        row()
            .key("controls")
            .w_full()
            .gap(10.0)
            .center_y()
            .child(button)
            .child(time("elapsed", shown))
            .child(scrubber(p, &meeting.id, progress, duration, shown))
            .child(time("length", duration)),
    );
    let error = audio
        .playback
        .as_ref()
        .filter(|playback| playback.id == meeting.id)
        .and_then(Playback::error);
    if let Some(error) = error {
        player = player.child(
            text(tr_fill("Could not play: {}", tr(error)))
                .key("play-error")
                .font(super::DESCRIPTION)
                .color(p.muted())
                .wrap(),
        );
    }
    if let Some(error) = audio
        .export
        .as_ref()
        .filter(|export| export.id == meeting.id)
        .and_then(|export| export.error.as_deref())
    {
        player = player.child(error_panel(
            p,
            "export-audio-error",
            tr("Audio could not be exported."),
            tr(error),
            Tone::Danger,
        ));
    }
    player
}

/// A rail that fills as the meeting plays. Pressing or dragging it moves
/// the time; letting go plays from there if it was playing.
fn scrubber(p: Paint, id: &str, progress: f32, duration: u64, shown: u64) -> El {
    let c = &p.k.c;
    let progress = progress.clamp(0.0, 1.0);
    let id = id.to_string();
    row()
        .key("scrubber")
        .role("slider")
        .label(tr("Audio position"))
        .range_value(f64::from(progress), 0.0, 1.0)
        .a11y_value(clock(shown))
        .grow(1.0)
        .shrink()
        .h(22.0)
        .center_y()
        .on_drag(move |ui: &mut Ui, drag: &mut Drag, _: &mut Tx| {
            if drag.cancelled() {
                ui.meetings.audio.scrub = None;
                return;
            }
            let at = (drag.x / drag.w.max(1.0)).clamp(0.0, 1.0);
            let ms = (f64::from(at) * duration as f64) as u64;
            if drag.phase == Phase::Up {
                ui.meetings.audio.scrub = None;
                let playing = ui
                    .meetings
                    .audio
                    .playback
                    .as_ref()
                    .is_some_and(|playback| playback.id == id && playback.playing());
                play_from(ui, &id, ms, playing);
            } else {
                ui.meetings.audio.scrub = Some(ms);
            }
        })
        .child(
            row()
                .key("rail")
                .grow(1.0)
                .h(4.0)
                .radius(2.0)
                .fill(c.layer_03)
                .child(
                    el().key("played")
                        .grow(progress.max(f32::EPSILON))
                        .h(4.0)
                        .radius(2.0)
                        .fill(c.icon_accent),
                )
                .child(el().key("left").grow((1.0 - progress).max(f32::EPSILON)))
                .a11y_hidden(),
        )
}

/// A transcript line that plays from its start when pressed. The caller
/// dims every line but the one playing.
pub(super) fn turn_row(id: &str, index: usize, turn: &Turn, body: Vec<El>) -> El {
    let id = id.to_string();
    let start = turn.start_ms;
    col()
        .key(format!("play-turn{index}"))
        .role("button")
        .label(tr("Play from here"))
        .description(turn.text.clone())
        .cursor(rune::Cursor::Pointer)
        .children(body)
        .on_press(move |ui: &mut Ui| play_from(ui, &id, start, true))
}

/// Export audio's menu item: writes the mix beside the Markdown on a
/// worker, then reveals it.
pub(super) fn export(ui: &mut Ui, id: String, markdown: PathBuf) {
    let Some(folder) = ui
        .meetings
        .meeting(&id)
        .map(|meeting| meeting.folder.clone())
    else {
        return;
    };
    let (sender, receiver) = channel();
    let waker = ui.modes.waker.clone();
    let started = std::thread::Builder::new()
        .name("hex-meeting-export".into())
        .spawn(move || {
            let _ = sender.send(crate::meeting::playback::export_beside(&folder, &markdown));
            if let Some(waker) = &waker {
                waker();
            }
        });
    ui.meetings.audio.export = Some(Export {
        id,
        error: started.as_ref().err().map(ToString::to_string),
        worker: started.ok().map(|_| receiver),
    });
}

/// Whether an Export audio runs; one at a time, so its answer is never
/// dropped for another meeting's.
pub(super) fn exporting(ui: &Ui) -> bool {
    ui.meetings
        .audio
        .export
        .as_ref()
        .is_some_and(|export| export.worker.is_some())
}

/// Before each frame: ends playback that lost its meeting, takes Export
/// audio's answer, and keeps a playing line in view. True when the pane
/// should redraw.
pub(super) fn poll(ui: &mut Ui, tx: &mut Tx) -> bool {
    let mut changed = poll_export(ui);
    let shown = ui.pane == Pane::Meetings
        && !ui.labs_open
        && ui.meetings.view == View::Recordings
        && ui.window.as_ref().is_none_or(rune::AppHandle::is_visible);
    let Some(id) = ui
        .meetings
        .audio
        .playback
        .as_ref()
        .map(|playback| playback.id.clone())
    else {
        return changed;
    };
    // An armed Delete closes the tracks first: Windows can't move a folder
    // with files still open.
    let keep = shown
        && ui.meetings.selected.as_deref() == Some(id.as_str())
        && ui.meetings.delete_armed.as_deref() != Some(id.as_str())
        && ui
            .meetings
            .meeting(&id)
            .is_some_and(|meeting| offered(ui, meeting));
    if !keep {
        let audio = &mut ui.meetings.audio;
        audio.playback = None;
        audio.scrub = None;
        audio.followed = None;
        return true;
    }
    let Some(playback) = ui.meetings.audio.playback.as_ref() else {
        return changed;
    };
    if playback.playing() || playback.error().is_some() {
        changed = true;
    }
    if !playback.playing() || ui.meetings.audio.scrub.is_some() {
        return changed;
    }
    tx.wake_after(FRAME);
    let position = playback.position_ms();
    let now = ui
        .meetings
        .meeting(&id)
        .and_then(|meeting| playing_turn(meeting.turns(), position));
    if now.is_some() && now != ui.meetings.audio.followed {
        ui.meetings.audio.followed = now;
        if let Some(index) = now {
            tx.scroll_center_in(TRANSCRIPT, format!("play-turn{index}"));
        }
    }
    changed
}

fn poll_export(ui: &mut Ui) -> bool {
    let Some(export) = ui.meetings.audio.export.as_mut() else {
        return false;
    };
    let result = match export.worker.as_ref().map(Receiver::try_recv) {
        Some(Ok(result)) => result,
        Some(Err(TryRecvError::Disconnected)) => Err(tr("Playback stopped.").to_string()),
        Some(Err(TryRecvError::Empty)) | None => return false,
    };
    export.worker = None;
    match result {
        Ok(path) => {
            export.error = None;
            super::ask(
                ui,
                super::MeetingRequest::Reveal(crate::meeting::RevealTarget::Markdown(path)),
            );
        }
        Err(error) => export.error = Some(error),
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn turn(start_ms: u64) -> Turn {
        Turn {
            mine: false,
            start_ms,
            text: "Hi.".into(),
            pause_before_ms: None,
            entries: Vec::new(),
        }
    }

    #[test]
    fn the_playing_line_is_the_last_one_started() {
        let turns = [turn(1_000), turn(4_000), turn(9_000)];
        assert_eq!(playing_turn(&turns, 0), None, "nothing said yet");
        assert_eq!(playing_turn(&turns, 1_000), Some(0));
        assert_eq!(playing_turn(&turns, 8_999), Some(1));
        assert_eq!(playing_turn(&turns, 60_000), Some(2));
    }

    #[test]
    fn the_player_clock_grows_hours_only_when_needed() {
        assert_eq!(clock(3_400), "0:03");
        assert_eq!(clock(247_000), "4:07");
        assert_eq!(clock(3_909_000), "1:05:09");
    }
}
