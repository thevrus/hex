//! Hearing a saved meeting, and exporting its audio beside its Markdown
//! (Labs › Play meeting audio).
//!
//! A meeting keeps two mono float tracks on one capture clock:
//! `microphone.wav` (You) and `system.wav` (Them). [`Mix`] reads both from
//! disk as it plays, resamples one onto the other's rate when they differ,
//! and sums them under a soft limiter, so an hour-long call never sits in
//! memory. One worker thread owns the output device, opened on the first
//! Play and never before, and takes the pane's commands; the pane reads the
//! position and state it publishes.

use std::fs::File;
use std::io::{self, BufReader, BufWriter};
use std::num::{NonZeroU16, NonZeroU32};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender, channel};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use hound::{SampleFormat, WavReader, WavSpec, WavWriter};
use rodio::source::SeekError;
use rodio::{DeviceSinkBuilder, MixerDeviceSink, Player, Source};

/// How often a playing meeting publishes its position.
const TICK: Duration = Duration::from_millis(30);
/// How long an idle worker waits for a command before checking again.
const IDLE: Duration = Duration::from_millis(500);

const GONE: &str = "The recording is gone.";
const UNREADABLE: &str = "The recording could not be read.";

/// One mono track read on demand and resampled onto the mix's rate.
struct Track {
    reader: WavReader<BufReader<File>>,
    format: SampleFormat,
    rate: u32,
    frames: u64,
    /// Source frames per output frame.
    step: f64,
    /// Where the next output frame falls, in source frames.
    position: f64,
    /// The source frame `window.0` holds; `window.1` is the one after it.
    base: u64,
    window: (f32, f32),
}

impl Track {
    /// The track at `path`, or `None` when it is missing, empty, or not one
    /// mono track Hex wrote.
    fn open(path: &Path) -> Option<Self> {
        let reader = WavReader::open(path).ok()?;
        let spec = reader.spec();
        let frames = u64::from(reader.duration());
        let supported = matches!(
            (spec.sample_format, spec.bits_per_sample),
            (SampleFormat::Float, 32) | (SampleFormat::Int, 16)
        );
        if spec.channels != 1 || frames == 0 || spec.sample_rate == 0 || !supported {
            return None;
        }
        let mut track = Self {
            reader,
            format: spec.sample_format,
            rate: spec.sample_rate,
            frames,
            step: 1.0,
            position: 0.0,
            base: 0,
            window: (0.0, 0.0),
        };
        track.window = (track.read(), track.read());
        Some(track)
    }

    fn read(&mut self) -> f32 {
        match self.format {
            SampleFormat::Float => self.reader.samples::<f32>().next(),
            SampleFormat::Int => self
                .reader
                .samples::<i16>()
                .next()
                .map(|sample| sample.map(|sample| f32::from(sample) / f32::from(i16::MAX))),
        }
        .and_then(Result::ok)
        .unwrap_or(0.0)
    }

    fn seconds(&self) -> f64 {
        self.frames as f64 / f64::from(self.rate)
    }

    /// Moves to `seconds` from the start.
    fn seek(&mut self, seconds: f64) -> io::Result<()> {
        let position = (seconds * f64::from(self.rate)).clamp(0.0, self.frames as f64);
        let base = (position.floor() as u64).min(self.frames);
        self.reader.seek(u32::try_from(base).unwrap_or(u32::MAX))?;
        self.base = base;
        self.position = position;
        self.window = (self.read(), self.read());
        Ok(())
    }

    /// The next output frame, linearly interpolated; `None` past the end.
    fn next(&mut self) -> Option<f32> {
        if self.position >= self.frames as f64 {
            return None;
        }
        let wanted = self.position.floor() as u64;
        while self.base < wanted {
            self.base += 1;
            self.window = (self.window.1, self.read());
        }
        let t = (self.position - self.base as f64) as f32;
        self.position += self.step;
        Some(self.window.0 + (self.window.1 - self.window.0) * t)
    }
}

/// Both of a meeting's tracks summed into one mono stream at the higher of
/// their rates.
pub struct Mix {
    tracks: Vec<Track>,
    rate: NonZeroU32,
    seconds: f64,
}

impl Mix {
    /// The meeting in `folder`, from whichever tracks hold audio. An
    /// imported meeting has only `system.wav`.
    pub fn open(folder: &Path) -> Result<Self, &'static str> {
        let present = ["microphone.wav", "system.wav"]
            .iter()
            .any(|file| folder.join(file).is_file());
        if !present {
            return Err(GONE);
        }
        let mut tracks: Vec<Track> = ["microphone.wav", "system.wav"]
            .iter()
            .filter_map(|file| Track::open(&folder.join(file)))
            .collect();
        let rate = tracks.iter().map(|track| track.rate).max().unwrap_or(0);
        let rate = NonZeroU32::new(rate).ok_or(UNREADABLE)?;
        for track in &mut tracks {
            track.step = f64::from(track.rate) / f64::from(rate.get());
        }
        let seconds = tracks.iter().map(Track::seconds).fold(0.0, f64::max);
        Ok(Self {
            tracks,
            rate,
            seconds,
        })
    }

    pub fn duration(&self) -> Duration {
        Duration::from_secs_f64(self.seconds)
    }

    /// Moves every track to `to` from the start.
    pub fn seek(&mut self, to: Duration) -> io::Result<()> {
        let seconds = to.as_secs_f64().min(self.seconds);
        for track in &mut self.tracks {
            track.seek(seconds)?;
        }
        Ok(())
    }
}

/// Sums above 0.8 bend smoothly toward 1.0 instead of clipping, for the
/// rare moment both sides are loud at once.
fn limit(sample: f32) -> f32 {
    const KNEE: f32 = 0.8;
    let level = sample.abs();
    if level <= KNEE {
        return sample;
    }
    let headroom = 1.0 - KNEE;
    sample.signum() * (KNEE + headroom * ((level - KNEE) / headroom).tanh())
}

impl Iterator for Mix {
    type Item = f32;

    fn next(&mut self) -> Option<f32> {
        let mut any = false;
        let mut sum = 0.0;
        for track in &mut self.tracks {
            if let Some(sample) = track.next() {
                any = true;
                sum += sample;
            }
        }
        any.then(|| limit(sum))
    }
}

impl Source for Mix {
    fn current_span_len(&self) -> Option<usize> {
        None
    }

    fn channels(&self) -> NonZeroU16 {
        NonZeroU16::MIN
    }

    fn sample_rate(&self) -> NonZeroU32 {
        self.rate
    }

    fn total_duration(&self) -> Option<Duration> {
        Some(self.duration())
    }

    fn try_seek(&mut self, pos: Duration) -> Result<(), SeekError> {
        self.seek(pos)
            .map_err(|error| SeekError::Other(Arc::new(error)))
    }
}

enum Command {
    Play,
    Pause,
    Seek(Duration),
}

/// What the worker last published.
#[derive(Default)]
struct Shared {
    position_ms: AtomicU64,
    duration_ms: AtomicU64,
    playing: AtomicBool,
    error: Mutex<Option<&'static str>>,
}

/// One meeting's player, owned by the pane while that meeting shows.
/// Dropping it closes the worker, which stops the sound and releases the
/// output device.
pub struct Playback {
    /// The meeting it plays.
    pub id: String,
    commands: Sender<Command>,
    shared: Arc<Shared>,
}

impl Playback {
    /// A player for the meeting in `folder`, paused at its start. The
    /// worker reads the tracks' lengths at once but opens no device until
    /// [`Playback::play`]; `wake` asks the window to redraw when the state
    /// changes on its own (the end, a failure).
    pub fn new(id: String, folder: PathBuf, wake: impl Fn() + Send + 'static) -> Self {
        let (commands, receiver) = channel();
        let shared = Arc::new(Shared::default());
        let worker = Arc::clone(&shared);
        let started = thread::Builder::new()
            .name("hex-meeting-playback".into())
            .spawn(move || run(&folder, &worker, &receiver, &wake));
        if started.is_err() {
            *lock(&shared.error) = Some("Playback stopped.");
        }
        Self {
            id,
            commands,
            shared,
        }
    }

    pub fn play(&self) {
        *lock(&self.shared.error) = None;
        self.shared.playing.store(true, Ordering::Release);
        let _ = self.commands.send(Command::Play);
    }

    pub fn pause(&self) {
        self.shared.playing.store(false, Ordering::Release);
        let _ = self.commands.send(Command::Pause);
    }

    /// Moves to `ms` from the start, playing or paused as before.
    pub fn seek(&self, ms: u64) {
        self.shared.position_ms.store(ms, Ordering::Release);
        let _ = self.commands.send(Command::Seek(Duration::from_millis(ms)));
    }

    pub fn playing(&self) -> bool {
        self.shared.playing.load(Ordering::Acquire)
    }

    pub fn position_ms(&self) -> u64 {
        self.shared.position_ms.load(Ordering::Acquire)
    }

    /// The meeting's length, once the worker has read its tracks.
    pub fn duration_ms(&self) -> Option<u64> {
        Some(self.shared.duration_ms.load(Ordering::Acquire)).filter(|ms| *ms > 0)
    }

    /// Why it could not play, until the next Play.
    pub fn error(&self) -> Option<&'static str> {
        *lock(&self.shared.error)
    }
}

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|error| error.into_inner())
}

fn millis(duration: Duration) -> u64 {
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}

/// The output device and the player on it, opened by the first Play.
struct Output {
    _sink: MixerDeviceSink,
    player: Player,
}

/// The worker: takes commands until the pane drops its [`Playback`].
fn run(folder: &Path, shared: &Shared, commands: &Receiver<Command>, wake: &dyn Fn()) {
    let duration = match Mix::open(folder) {
        Ok(mix) => mix.duration(),
        Err(error) => {
            *lock(&shared.error) = Some(error);
            shared.playing.store(false, Ordering::Release);
            wake();
            return;
        }
    };
    shared
        .duration_ms
        .store(millis(duration).max(1), Ordering::Release);
    wake();
    let mut output: Option<Output> = None;
    // Where the next Play starts while nothing is queued.
    let mut position = Duration::ZERO;
    // Whether this worker started the sound now queued. Only the worker
    // decides the end: the pane sets `playing` before its Play arrives.
    let mut started = false;
    loop {
        let sounding = output
            .as_ref()
            .is_some_and(|output| !output.player.is_paused() && !output.player.empty());
        match commands.recv_timeout(if sounding { TICK } else { IDLE }) {
            Ok(Command::Play) => {
                if position >= duration {
                    position = Duration::ZERO;
                }
                match start(folder, &mut output, position) {
                    Ok(()) => {
                        started = true;
                        shared.playing.store(true, Ordering::Release);
                    }
                    Err(error) => {
                        started = false;
                        *lock(&shared.error) = Some(error);
                        shared.playing.store(false, Ordering::Release);
                        wake();
                    }
                }
            }
            Ok(Command::Pause) => {
                started = false;
                if let Some(output) = &output {
                    output.player.pause();
                    if !output.player.empty() {
                        position = output.player.get_pos();
                    }
                }
            }
            Ok(Command::Seek(to)) => {
                position = to.min(duration);
                if let Some(output) = output.as_ref().filter(|output| !output.player.empty()) {
                    let _ = output.player.try_seek(position);
                }
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => return,
        }
        let Some(output) = &output else {
            continue;
        };
        if output.player.empty() {
            // The end: the pane shows Play again at the end, and the next
            // Play starts over.
            if std::mem::take(&mut started) {
                shared.playing.store(false, Ordering::Release);
                position = duration;
                shared
                    .position_ms
                    .store(millis(duration), Ordering::Release);
                wake();
            }
        } else if !output.player.is_paused() {
            position = output.player.get_pos().min(duration);
            shared
                .position_ms
                .store(millis(position), Ordering::Release);
        }
    }
}

/// Opens the device on the first Play, queues the meeting from `from` when
/// nothing is queued, and plays.
fn start(folder: &Path, output: &mut Option<Output>, from: Duration) -> Result<(), &'static str> {
    if output.is_none() {
        let mut sink =
            DeviceSinkBuilder::open_default_sink().map_err(|_| "No audio output is available.")?;
        sink.log_on_drop(false);
        let player = Player::connect_new(sink.mixer());
        *output = Some(Output {
            _sink: sink,
            player,
        });
    }
    let Some(output) = output.as_ref() else {
        return Err("Playback stopped.");
    };
    if output.player.empty() {
        let mut mix = Mix::open(folder)?;
        mix.seek(from).map_err(|_| UNREADABLE)?;
        // Queued paused, so nothing sounds before the player's position is
        // set to where the source already is (it counts from its source).
        output.player.pause();
        output.player.append(mix);
        let _ = output.player.try_seek(from);
        output.player.play();
    } else {
        output.player.play();
    }
    Ok(())
}

/// Writes the meeting's mixed audio as 16-bit WAV beside `markdown`, named
/// after it (`Design sync.wav`, then `Design sync 2.wav`, never over a file
/// that exists), and returns its path. Blocks; run it off the UI thread.
pub fn export_beside(folder: &Path, markdown: &Path) -> Result<PathBuf, String> {
    let mut mix = Mix::open(folder).map_err(str::to_string)?;
    let directory = markdown.parent().unwrap_or(Path::new("."));
    let stem = markdown
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("Meeting");
    let partial = directory.join(format!(".{stem}.wav.partial"));
    let written = (|| -> Result<(), hound::Error> {
        let mut writer = WavWriter::new(
            BufWriter::new(File::create(&partial)?),
            WavSpec {
                channels: 1,
                sample_rate: mix.rate.get(),
                bits_per_sample: 16,
                sample_format: SampleFormat::Int,
            },
        )?;
        for sample in &mut mix {
            writer.write_sample((sample.clamp(-1.0, 1.0) * f32::from(i16::MAX)) as i16)?;
        }
        writer.finalize()
    })();
    if let Err(error) = written {
        let _ = std::fs::remove_file(&partial);
        return Err(error.to_string());
    }
    let placed =
        crate::durable::sync_file(&partial).and_then(|()| place(&partial, directory, stem));
    let _ = std::fs::remove_file(&partial);
    placed.map_err(|error| error.to_string())
}

/// Links the finished `partial` in as the first free `<stem>.wav`,
/// `<stem> 2.wav`, …: a hard link fails rather than replace a file that
/// appeared meanwhile. A volume without hard links gets a rename to a name
/// that was free a moment before.
fn place(partial: &Path, directory: &Path, stem: &str) -> io::Result<PathBuf> {
    for n in 1..1_000 {
        let target = directory.join(if n == 1 {
            format!("{stem}.wav")
        } else {
            format!("{stem} {n}.wav")
        });
        match std::fs::hard_link(partial, &target) {
            Ok(()) => return Ok(target),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
            Err(_) if !target.exists() => {
                std::fs::rename(partial, &target)?;
                return Ok(target);
            }
            Err(error) => return Err(error),
        }
    }
    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        "every export name is taken",
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn folder(name: &str) -> PathBuf {
        let folder = std::env::temp_dir().join(format!(
            "hex-meeting-playback-{name}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&folder);
        std::fs::create_dir_all(&folder).unwrap();
        folder
    }

    fn write(path: &Path, rate: u32, samples: &[f32]) {
        let mut writer = WavWriter::create(
            path,
            WavSpec {
                channels: 1,
                sample_rate: rate,
                bits_per_sample: 32,
                sample_format: SampleFormat::Float,
            },
        )
        .unwrap();
        for sample in samples {
            writer.write_sample(*sample).unwrap();
        }
        writer.finalize().unwrap();
    }

    #[test]
    fn both_sides_play_as_one_track_at_the_faster_rate() {
        let folder = folder("mix");
        write(&folder.join("microphone.wav"), 8, &[0.25; 8]);
        // Half the rate for half the frames: the same one second.
        write(&folder.join("system.wav"), 4, &[0.5; 4]);
        let mix = Mix::open(&folder).unwrap();
        assert_eq!(mix.sample_rate().get(), 8);
        assert_eq!(mix.duration(), Duration::from_secs(1));
        let samples: Vec<f32> = mix.collect();
        let _ = std::fs::remove_dir_all(&folder);
        assert_eq!(samples.len(), 8, "one second at the faster rate");
        assert!(
            samples[..6]
                .iter()
                .all(|sample| (sample - 0.75).abs() < 1e-6)
        );
    }

    #[test]
    fn an_imported_meeting_plays_its_one_track() {
        let folder = folder("imported");
        write(&folder.join("system.wav"), 16_000, &[0.1, 0.2, 0.3]);
        write(&folder.join("microphone.wav"), 16_000, &[]);
        let samples: Vec<f32> = Mix::open(&folder).unwrap().collect();
        let _ = std::fs::remove_dir_all(&folder);
        assert_eq!(samples.len(), 3);
        assert!((samples[2] - 0.3).abs() < 1e-6);
    }

    #[test]
    fn a_meeting_without_tracks_is_gone() {
        let folder = folder("gone");
        let error = Mix::open(&folder).err();
        let _ = std::fs::remove_dir_all(&folder);
        assert_eq!(error, Some(GONE));
    }

    #[test]
    fn seeking_lands_on_the_frame_at_that_time() {
        let folder = folder("seek");
        let ramp: Vec<f32> = (0..10).map(|n| n as f32 / 20.0).collect();
        write(&folder.join("system.wav"), 10, &ramp);
        let mut mix = Mix::open(&folder).unwrap();
        mix.seek(Duration::from_millis(500)).unwrap();
        let first = mix.next().unwrap();
        let rest = mix.count();
        let _ = std::fs::remove_dir_all(&folder);
        assert!((first - 0.25).abs() < 1e-6, "{first}");
        assert_eq!(rest, 4);
    }

    #[test]
    fn loud_double_talk_bends_under_full_scale() {
        assert_eq!(limit(0.5), 0.5);
        assert!(limit(1.6) < 1.0 && limit(1.6) > 0.95);
        assert!(limit(-1.6) > -1.0);
    }

    #[test]
    fn export_lands_beside_the_markdown_and_never_overwrites() {
        let folder = folder("export");
        write(&folder.join("system.wav"), 16_000, &[0.5; 160]);
        let markdown = folder.join("2026-11-23 Design sync.md");
        std::fs::write(&markdown, "# Design sync\n").unwrap();
        let first = export_beside(&folder, &markdown).unwrap();
        let second = export_beside(&folder, &markdown).unwrap();
        let reader = WavReader::open(&first).unwrap();
        let spec = reader.spec();
        let frames = reader.duration();
        let _ = std::fs::remove_dir_all(&folder);
        assert_eq!(first.file_name().unwrap(), "2026-11-23 Design sync.wav");
        assert_eq!(second.file_name().unwrap(), "2026-11-23 Design sync 2.wav");
        assert_eq!((spec.bits_per_sample, spec.sample_rate), (16, 16_000));
        assert_eq!(frames, 160);
    }
}
