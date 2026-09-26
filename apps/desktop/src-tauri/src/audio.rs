//! Local MP3 library and queue rules. Actual device I/O is an adapter boundary;
//! a missing file/output device never changes focus-session state.
use id3::TagLike;
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use std::{
    fs::File,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc, Mutex,
    },
    time::SystemTime,
    time::{Duration, Instant},
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Track {
    pub path: PathBuf,
    pub title: String,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub available: bool,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PlaybackHealth {
    Ready,
    WaitingForOutputDevice,
    MissingFile,
}
#[derive(Clone, Debug)]
pub struct AudioEngine {
    pub queue: Vec<Track>,
    pub current: Option<usize>,
    pub volume: u8,
    pub playing: bool,
    pub health: PlaybackHealth,
}
impl Default for AudioEngine {
    fn default() -> Self {
        Self {
            queue: Vec::new(),
            current: None,
            volume: 80,
            playing: false,
            health: PlaybackHealth::Ready,
        }
    }
}
impl AudioEngine {
    pub fn import_file(path: impl Into<PathBuf>) -> Option<Track> {
        let path = path.into();
        if path.extension()?.to_str()?.eq_ignore_ascii_case("mp3") {
            Some(track_from_path(path))
        } else {
            None
        }
    }
    pub fn scan_folder(folder: impl AsRef<Path>) -> Vec<Track> {
        let mut paths = walkdir::WalkDir::new(folder)
            .follow_links(false)
            .into_iter()
            .filter_map(Result::ok)
            .filter(|entry| entry.file_type().is_file())
            .map(|entry| entry.into_path())
            .filter(|path| {
                path.extension()
                    .and_then(|ext| ext.to_str())
                    .is_some_and(|ext| ext.eq_ignore_ascii_case("mp3"))
            })
            .collect::<Vec<_>>();
        paths.sort_by(|a, b| a.to_string_lossy().cmp(&b.to_string_lossy()));
        paths.into_iter().map(track_from_path).collect()
    }
    pub fn reconcile_folder(&mut self, source: &Path, discovered: Vec<Track>) {
        let current_path = self
            .current
            .and_then(|index| self.queue.get(index))
            .map(|track| track.path.clone());
        let discovered_paths: std::collections::BTreeSet<_> =
            discovered.iter().map(|t| t.path.clone()).collect();
        self.queue
            .retain(|t| !t.path.starts_with(source) || discovered_paths.contains(&t.path));
        for track in &mut self.queue {
            if let Some(updated) = discovered.iter().find(|item| item.path == track.path) {
                *track = updated.clone();
            }
        }
        let existing: std::collections::BTreeSet<_> =
            self.queue.iter().map(|t| t.path.clone()).collect();
        self.queue.extend(
            discovered
                .into_iter()
                .filter(|t| !existing.contains(&t.path)),
        );
        if let Some(path) = current_path {
            self.current = self.queue.iter().position(|track| track.path == path);
            if self.current.is_none() {
                self.playing = false;
                self.health = PlaybackHealth::MissingFile;
            }
        }
    }
    pub fn set_output_available(&mut self, available: bool) {
        self.health = if available {
            PlaybackHealth::Ready
        } else {
            PlaybackHealth::WaitingForOutputDevice
        };
        if !available {
            self.playing = false;
        }
    }
    pub fn play(&mut self) {
        if self.current.is_some() && self.health == PlaybackHealth::Ready {
            self.playing = true;
        }
    }
    pub fn pause(&mut self) {
        self.playing = false;
    }
    pub fn previous(&mut self) {
        if self.queue.is_empty() {
            self.current = None;
            self.playing = false;
        } else {
            self.current = Some(
                self.current
                    .map_or(0, |index| (index + self.queue.len() - 1) % self.queue.len()),
            );
        }
    }
    pub fn next(&mut self) {
        if self.queue.is_empty() {
            self.current = None;
            self.playing = false;
        } else {
            self.current = Some(self.current.map_or(0, |i| (i + 1) % self.queue.len()));
        }
    }
    /// Natural completion stops at the end of the queue: repeat is not enabled.
    pub fn advance_after_finish(&mut self) -> bool {
        if let Some(next) = self.current.and_then(|index| index.checked_add(1)) {
            if next < self.queue.len() {
                self.current = Some(next);
                return true;
            }
        }
        self.playing = false;
        false
    }
    pub fn set_volume(&mut self, volume: u8) {
        self.volume = volume.min(100);
    }
}
fn track_from_path(path: PathBuf) -> Track {
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("Unknown track");
    let (fallback_artist, fallback_title) = stem
        .split_once(" - ")
        .map_or((None, stem.to_string()), |(a, t)| {
            (Some(a.to_string()), t.to_string())
        });
    let tag = id3::Tag::read_from_path(&path).ok();
    Track {
        available: path.exists(),
        path,
        title: tag
            .as_ref()
            .and_then(|value| value.title())
            .filter(|value| !value.trim().is_empty())
            .map_or(fallback_title, str::to_string),
        artist: tag
            .as_ref()
            .and_then(|value| value.artist())
            .filter(|value| !value.trim().is_empty())
            .map(str::to_string)
            .or(fallback_artist),
        album: tag
            .as_ref()
            .and_then(|value| value.album())
            .filter(|value| !value.trim().is_empty())
            .map(str::to_string),
    }
}

/// Production output adapter selected for P2-09. Construction is fallible so
/// the focus session can continue in `waiting_for_output_device` without music.
pub struct RodioPlayer {
    _output: rodio::MixerDeviceSink,
    player: rodio::Player,
    output_failed: Arc<AtomicBool>,
    progress: Mutex<PlaybackProgress>,
}

struct PlaybackProgress {
    position: Duration,
    observed_at: Instant,
}
impl PlaybackProgress {
    fn observe(&mut self, position: Duration, now: Instant, playing: bool) -> bool {
        if !playing || position != self.position {
            self.position = position;
            self.observed_at = now;
        }
        playing && now.saturating_duration_since(self.observed_at) >= Duration::from_secs(3)
    }
}

impl RodioPlayer {
    pub fn open_default() -> Result<Self, String> {
        let output_failed = Arc::new(AtomicBool::new(false));
        let failed = output_failed.clone();
        let output = rodio::DeviceSinkBuilder::from_default_device()
            .map_err(|error| error.to_string())?
            .with_error_callback(move |error| {
                // CPAL recovers ordinary underruns itself. A transient scheduling
                // delay must not turn into a permanent device-loss state.
                if !matches!(error, rodio::cpal::StreamError::BufferUnderrun) {
                    failed.store(true, Ordering::Release);
                }
            })
            .open_sink_or_fallback()
            .map_err(|error| error.to_string())?;
        let player = rodio::Player::connect_new(output.mixer());
        Ok(Self {
            _output: output,
            player,
            output_failed,
            progress: Mutex::new(PlaybackProgress {
                position: Duration::ZERO,
                observed_at: Instant::now(),
            }),
        })
    }
    pub fn output_description(&self) -> String {
        "Default system output".to_string()
    }
    pub fn output_failed(&self) -> bool {
        self.output_failed.load(Ordering::Acquire)
    }
    /// Some ALSA/PipeWire failures leave the stream polling indefinitely without
    /// an error callback. Detect a stopped playback clock while sound is requested.
    pub fn check_output(&self, playing: bool) -> bool {
        if let Ok(mut progress) = self.progress.lock() {
            if progress.observe(self.player.get_pos(), Instant::now(), playing) {
                self.output_failed.store(true, Ordering::Release);
            }
        }
        self.output_failed()
    }
    fn reset_progress(&self) {
        if let Ok(mut progress) = self.progress.lock() {
            progress.position = self.player.get_pos();
            progress.observed_at = Instant::now();
        }
    }
    pub fn finished(&self) -> bool {
        self.player.empty()
    }
    pub fn play_file(&self, path: &Path) -> Result<(), String> {
        let file = File::open(path).map_err(|error| error.to_string())?;
        let source = rodio::Decoder::try_from(file).map_err(|error| error.to_string())?;
        self.player.stop();
        self.player.append(source);
        self.player.play();
        self.reset_progress();
        Ok(())
    }
    pub fn pause(&self) {
        self.player.pause();
    }
    pub fn resume(&self) {
        self.reset_progress();
        self.player.play();
    }
    pub fn set_volume(&self, volume: u8) {
        self.player.set_volume(f32::from(volume.min(100)) / 100.0);
    }
    pub fn stop(&self) {
        self.player.stop();
    }
    pub fn fade_and_stop(&self, duration: Duration) {
        let start = self.player.volume();
        for step in (0..20).rev() {
            self.player.set_volume(start * step as f32 / 20.0);
            std::thread::sleep(duration / 20);
        }
        self.player.stop();
        self.player.set_volume(start);
    }
}

pub fn watch_folder(
    path: &Path,
) -> Result<(RecommendedWatcher, mpsc::Receiver<()>), notify::Error> {
    let (sender, receiver) = mpsc::sync_channel(1);
    let mut watcher = notify::recommended_watcher(move |event: notify::Result<notify::Event>| {
        // Reading ID3 metadata produces Access events on Linux. Treating those
        // reads as changes makes every rescan schedule another rescan forever.
        if event.is_ok_and(|event| !matches!(event.kind, notify::EventKind::Access(_))) {
            let _ = sender.try_send(());
        }
    })?;
    watcher.watch(path, RecursiveMode::Recursive)?;
    Ok((watcher, receiver))
}
pub fn scan_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    #[test]
    fn mp3_filter_and_filename_fallback() {
        let d = std::env::temp_dir().join(format!("deepify-audio-{}", scan_timestamp()));
        fs::create_dir_all(&d).unwrap();
        fs::write(d.join("Artist - Song.mp3"), b"not audio").unwrap();
        fs::write(d.join("skip.wav"), b"x").unwrap();
        let tracks = AudioEngine::scan_folder(&d);
        assert_eq!(tracks.len(), 1);
        assert_eq!(tracks[0].artist.as_deref(), Some("Artist"));
        assert_eq!(tracks[0].title, "Song");
        fs::remove_dir_all(d).unwrap();
    }
    #[test]
    fn device_loss_is_nonfatal() {
        let a_track = Track {
            path: PathBuf::from("x.mp3"),
            title: "x".into(),
            artist: None,
            album: None,
            available: true,
        };
        let mut a = AudioEngine {
            queue: vec![a_track],
            current: Some(0),
            ..Default::default()
        };
        a.set_output_available(false);
        assert_eq!(a.health, PlaybackHealth::WaitingForOutputDevice);
        assert!(!a.playing);
    }

    #[test]
    fn output_watchdog_distinguishes_stall_from_pause_and_progress() {
        let now = Instant::now();
        let mut watch = PlaybackProgress {
            position: Duration::ZERO,
            observed_at: now,
        };
        assert!(!watch.observe(Duration::from_secs(1), now + Duration::from_secs(2), true));
        assert!(!watch.observe(Duration::from_secs(1), now + Duration::from_secs(4), true));
        assert!(watch.observe(Duration::from_secs(1), now + Duration::from_secs(5), true));
        assert!(!watch.observe(Duration::from_secs(1), now + Duration::from_secs(50), false));
        assert!(!watch.observe(Duration::from_secs(1), now + Duration::from_secs(51), true));
        assert!(!watch.observe(Duration::from_secs(2), now + Duration::from_secs(52), true));
    }

    #[test]
    fn id3_metadata_precedes_filename_with_blank_tag_fallbacks() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("Filename artist - Filename title.mp3");
        fs::write(&path, b"fixture").unwrap();
        let mut tag = id3::Tag::new();
        tag.set_title("Tagged title");
        tag.set_artist("Tagged artist");
        tag.set_album("Tagged album");
        tag.write_to_path(&path, id3::Version::Id3v24).unwrap();
        let track = AudioEngine::import_file(&path).unwrap();
        assert_eq!(track.title, "Tagged title");
        assert_eq!(track.artist.as_deref(), Some("Tagged artist"));
        assert_eq!(track.album.as_deref(), Some("Tagged album"));
        tag.set_title(" ");
        tag.set_artist("");
        tag.write_to_path(&path, id3::Version::Id3v24).unwrap();
        let track = AudioEngine::import_file(&path).unwrap();
        assert_eq!(track.title, "Filename title");
        assert_eq!(track.artist.as_deref(), Some("Filename artist"));
    }

    #[test]
    fn natural_completion_advances_then_stops_without_repeating() {
        let mut audio = AudioEngine {
            queue: vec![
                track_from_path("a.mp3".into()),
                track_from_path("b.mp3".into()),
            ],
            current: Some(0),
            playing: true,
            ..Default::default()
        };
        assert!(audio.advance_after_finish());
        assert_eq!(audio.current, Some(1));
        assert!(!audio.advance_after_finish());
        assert!(!audio.playing);
        assert_eq!(audio.current, Some(1));
    }

    #[test]
    fn scanning_metadata_does_not_trigger_another_folder_rescan() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("song.mp3"), b"fixture").unwrap();
        let (_watcher, events) = watch_folder(dir.path()).unwrap();
        assert_eq!(AudioEngine::scan_folder(dir.path()).len(), 1);
        assert!(matches!(
            events.recv_timeout(Duration::from_millis(250)),
            Err(mpsc::RecvTimeoutError::Timeout)
        ));
    }

    #[test]
    fn recursive_watcher_reports_file_changes() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir(dir.path().join("nested")).unwrap();
        let (_watcher, events) = watch_folder(dir.path()).unwrap();
        let path = dir.path().join("nested/new.mp3");
        fs::write(&path, b"fixture").unwrap();
        events.recv_timeout(Duration::from_secs(3)).unwrap();
        assert_eq!(AudioEngine::scan_folder(dir.path()).len(), 1);
        while events.try_recv().is_ok() {}
        fs::remove_file(path).unwrap();
        events.recv_timeout(Duration::from_secs(3)).unwrap();
        assert!(AudioEngine::scan_folder(dir.path()).is_empty());
    }

    #[test]
    fn scan_does_not_follow_directory_symlink_cycles() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir(dir.path().join("nested")).unwrap();
        fs::write(dir.path().join("nested/song.mp3"), b"fixture").unwrap();
        std::os::unix::fs::symlink(dir.path(), dir.path().join("nested/loop")).unwrap();
        assert_eq!(AudioEngine::scan_folder(dir.path()).len(), 1);
    }

    #[test]
    fn rescan_keeps_survivor_order_and_selection_then_appends_new_tracks() {
        let track = |name: &str| track_from_path(PathBuf::from(format!("/library/{name}.mp3")));
        let mut audio = AudioEngine {
            queue: vec![track("z"), track("b"), track("c")],
            current: Some(1),
            playing: true,
            ..Default::default()
        };
        audio.reconcile_folder(
            Path::new("/library"),
            vec![track("a"), track("b"), track("z")],
        );
        assert_eq!(
            audio
                .queue
                .iter()
                .map(|t| t.title.as_str())
                .collect::<Vec<_>>(),
            ["z", "b", "a"]
        );
        assert_eq!(audio.current, Some(1));
        assert!(audio.playing);
        audio.reconcile_folder(Path::new("/library"), vec![track("a"), track("z")]);
        assert!(audio.current.is_none());
        assert!(!audio.playing);
        assert_eq!(audio.health, PlaybackHealth::MissingFile);
    }

    #[test]
    #[ignore = "requires DEEPIFY_AUDIO_FIXTURE and a live system output device"]
    fn production_adapter_decodes_and_controls_real_mp3() {
        let fixture = std::env::var_os("DEEPIFY_AUDIO_FIXTURE")
            .map(PathBuf::from)
            .expect("DEEPIFY_AUDIO_FIXTURE must point to a real MP3");
        let player = RodioPlayer::open_default().expect("system output device must be available");
        player.set_volume(5);
        player
            .play_file(&fixture)
            .expect("Rodio/Symphonia must decode the real MP3 fixture");
        std::thread::sleep(Duration::from_millis(100));
        player.pause();
        player.resume();
        player.stop();
    }
}
