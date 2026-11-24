use super::*;
use crate::rune_ui::Service;
use crate::rune_ui::platform::PlatformHost;

/// An imported meeting has one track: every turn is Speaker, not Them.
#[test]
fn an_imported_meeting_labels_its_one_track_speaker() {
    let turn = |mine| Turn {
        mine,
        start_ms: 1_000,
        text: "Hi.".into(),
        pause_before_ms: None,
        entries: Vec::new(),
    };
    assert!(transcript_markdown(&[turn(false)], true).contains("**Speaker** · 00:01"));
    assert!(transcript_markdown(&[turn(true)], true).contains("**Speaker**"));
    assert!(transcript_markdown(&[turn(true)], false).contains("**You**"));
    assert!(transcript_markdown(&[turn(false)], false).contains("**Them**"));
}

/// Linux and Windows carry meeting requests out in the window's own
/// process; the OS host ignored them, so Stop, Rename, and Trash from
/// the pane did nothing and reported nothing (#566).
#[test]
fn pane_requests_reach_the_window_s_own_meetings() {
    let mut ui = Ui::with(
        crate::app_settings::AppSettings::default(),
        Service::Fixture(Vec::new()),
    );
    assert!(matches!(ui.host, PlatformHost::Local(_)));
    ui.meetings = MeetingsPane::fixture();
    // No meeting controller runs in tests, so the request is refused
    // rather than silently accepted.
    stop(&mut ui);
    assert!(!ui.meetings.stop_pending);
    assert!(ui.meetings.action_error.is_some());
}

/// A known call service's icon, and the neutral one for any other or
/// no app (#1290).
#[test]
fn platform_icons_fall_back_to_a_neutral_one() {
    let pane = MeetingsPane::platforms_fixture();
    let name = |id: &str| platform_icon(pane.meeting(id).unwrap()).name;
    assert_eq!(name("preview-live"), "hex-provider-zoom");
    assert_eq!(name("preview-meet"), "hex-provider-meet");
    assert_eq!(name("preview-teams"), "hex-provider-teams");
    assert_eq!(name("preview-huddle"), "hex-provider-slack");
    assert_eq!(name("preview-design"), "hex-provider-call");
    assert_eq!(name("preview-failed"), "hex-provider-call");
}

#[test]
fn default_titles_hide_the_clock_the_row_already_shows() {
    let pane = MeetingsPane::fixture();
    assert_eq!(
        shown_title(pane.meeting("preview-live").unwrap()),
        "Zoom call"
    );
    assert_eq!(
        shown_title(pane.meeting("preview-failed").unwrap()),
        "Meeting"
    );
    assert_eq!(
        shown_title(pane.meeting("preview-notes").unwrap()),
        "Arcade launch plan"
    );
    let mut custom = pane.meeting("preview-notes").unwrap().clone();
    custom.title = "Standup — 11:12".into();
    assert_eq!(shown_title(&custom), "Standup — 11:12");
}

#[test]
fn failed_rows_show_one_line_reason_or_fallback() {
    let pane = MeetingsPane::fixture();
    assert_eq!(
        failure_reason(pane.meeting("preview-failed").unwrap()).as_deref(),
        Some("ScreenCaptureKit stopped the capture")
    );
    let mut bare = pane.meeting("preview-failed").unwrap().clone();
    bare.error = None;
    assert_eq!(failure_reason(&bare).as_deref(), Some("Couldn't record"));
    assert_eq!(failure_reason(pane.meeting("preview-notes").unwrap()), None);
}

/// Only a recording meeting's transcript follows its newest line (#1332).
#[test]
fn only_a_recording_transcript_follows() {
    let settings = crate::app_settings::AppSettings::default();
    let pane = MeetingsPane::fixture();
    let live = pane.meeting("preview-live").unwrap();
    let finished = pane.meeting("preview-notes").unwrap();
    let mut ui = Ui::with(settings, Service::Fixture(Vec::new()));
    assert!(follows_live_transcript(&ui, live));
    assert!(!follows_live_transcript(&ui, finished));
    ui.meetings.stop_pending = true;
    assert!(!follows_live_transcript(&ui, live));
}

/// Off a Mac the notes are written by OpenCode, so the progress line
/// names it, never Apple Intelligence (#1151).
#[test]
fn notes_progress_names_opencode_off_a_mac() {
    let ui = Ui::with(
        crate::app_settings::AppSettings::default(),
        Service::Fixture(Vec::new()),
    );
    let label = notes_running_label(&ui);
    assert!(label.contains("OpenCode"), "{label}");
    assert!(!label.contains("Apple Intelligence"), "{label}");
}
