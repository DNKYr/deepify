//! Local MP3 library and queue rules. Actual device I/O is an adapter boundary;
//! a missing file/output device never changes focus-session state.
use id3::TagLike;
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use std::{
    fs::File,
    path::{Path, PathBuf},
    sync::mpsc,
    time::Duration,
    time::SystemTime,
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
        let mut paths = Vec::new();
        visit(folder.as_ref(), &mut paths);
        paths.sort_by(|a, b| a.to_string_lossy().cmp(&b.to_string_lossy()));
        paths.into_iter().map(track_from_path).collect()
    }
    pub fn reconcile_folder(&mut self, source: &Path, discovered: Vec<Track>) {
        let discovered_paths: std::collections::BTreeSet<_> =
            discovered.iter().map(|t| t.path.clone()).collect();
        self.queue
            .retain(|t| !t.path.starts_with(source) || discovered_paths.contains(&t.path));
        let existing: std::collections::BTreeSet<_> =
            self.queue.iter().map(|t| t.path.clone()).collect();
        self.queue.extend(
            discovered
                .into_iter()
                .filter(|t| !existing.contains(&t.path)),
        );
        self.queue
            .sort_by(|a, b| a.path.to_string_lossy().cmp(&b.path.to_string_lossy()));
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
    pub fn set_volume(&mut self, volume: u8) {
        self.volume = volume.min(100);
    }
}
fn visit(folder: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(folder) else {
        return;
    };
    for entry in entries.flatten() {
        let p = entry.path();
        if p.is_dir() {
            visit(&p, out);
        } else if p
            .extension()
            .and_then(|x| x.to_str())
            .is_some_and(|x| x.eq_ignore_ascii_case("mp3"))
        {
            out.push(p);
        }
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
}

impl RodioPlayer {
    pub fn open_default() -> Result<Self, String> {
        let output =
            rodio::DeviceSinkBuilder::open_default_sink().map_err(|error| error.to_string())?;
        let player = rodio::Player::connect_new(output.mixer());
        Ok(Self {
            _output: output,
            player,
        })
    }
    pub fn output_description(&self) -> String {
        "Default system output".to_string()
    }
    pub fn play_file(&self, path: &Path) -> Result<(), String> {
        let file = File::open(path).map_err(|error| error.to_string())?;
        let source = rodio::Decoder::try_from(file).map_err(|error| error.to_string())?;
        self.player.stop();
        self.player.append(source);
        self.player.play();
        Ok(())
    }
    pub fn pause(&self) {
        self.player.pause();
    }
    pub fn resume(&self) {
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
    let (sender, receiver) = mpsc::channel();
    let mut watcher = notify::recommended_watcher(move |event: notify::Result<notify::Event>| {
        if event.is_ok() {
            let _ = sender.send(());
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
