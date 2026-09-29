use audio_player::{AsyncRodio, SeekDirection};
use audio_player::rodio::source::{SeekError, SineWave, Source};
use audio_player::rodio::{ChannelCount, SampleRate};
use futures::StreamExt;
use std::time::Duration;

fn device_available() -> bool {
    audio_player::rodio::DeviceSinkBuilder::open_default_sink().is_ok()
}

// The machine has a single output device; concurrent opens can fail, so
// the device-touching tests take this lock one at a time.
static DEVICE_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

// A source that plays silence forever but refuses every seek. Models a
// decoder whose format decodes fine yet cannot seek (no time base,
// fragmented container, demuxer error). The seek path must report the
// pre-seek position for it, never the requested one.
#[derive(Clone, Debug)]
struct Unseekable;

impl Iterator for Unseekable {
    type Item = f32;
    fn next(&mut self) -> Option<f32> {
        Some(0.0)
    }
}

impl Source for Unseekable {
    fn current_span_len(&self) -> Option<usize> {
        None
    }
    fn channels(&self) -> ChannelCount {
        ChannelCount::new(1).expect("one channel")
    }
    fn sample_rate(&self) -> SampleRate {
        SampleRate::new(44100).expect("44.1kHz")
    }
    fn total_duration(&self) -> Option<Duration> {
        None
    }
    fn try_seek(&mut self, _pos: Duration) -> Result<(), SeekError> {
        Err(SeekError::NotSupported {
            underlying_source: "seek_applied::Unseekable",
        })
    }
}

#[tokio::test]
async fn seek_reports_applied_position_on_success() {
    let _guard = DEVICE_LOCK.lock().expect("device lock");
    if !device_available() {
        eprintln!("SKIP: no audio output device");
        return;
    }
    let player = AsyncRodio::<SineWave, u32>::new();
    let mut stream = player.play_song(SineWave::new(440.0), 1);
    tokio::time::sleep(Duration::from_millis(300)).await;
    let reply = player
        .seek(Duration::from_secs(5), SeekDirection::Forward)
        .await
        .expect("seek reply");
    assert_eq!(reply.identifier, 1);
    assert!(
        reply.duration >= Duration::from_secs(5),
        "bar must show the seek target the sink reached, got {:?}",
        reply.duration
    );
    // Audio itself must keep playing from the new position, not the old one.
    let mut followed = Duration::ZERO;
    let deadline = tokio::time::Instant::now() + Duration::from_millis(800);
    while tokio::time::Instant::now() < deadline {
        if let Ok(Some(update)) =
            tokio::time::timeout(Duration::from_millis(300), stream.next()).await
        {
            if let audio_player::PlayUpdate::PlayProgress(d, 1) = update {
                followed = d;
            }
        }
    }
    assert!(
        followed >= Duration::from_secs(5),
        "audio must follow the bar after seek, got {:?}",
        followed
    );
}

#[tokio::test]
async fn seek_reports_pre_seek_position_on_failure() {
    let _guard = DEVICE_LOCK.lock().expect("device lock");
    if !device_available() {
        eprintln!("SKIP: no audio output device");
        return;
    }
    let player = AsyncRodio::<Unseekable, u32>::new();
    let _stream = player.play_song(Unseekable, 2);
    tokio::time::sleep(Duration::from_millis(300)).await;
    // Pause first: the mixer stops pulling, so nothing can paper over the
    // failed seek afterwards. The reply must still be the pre-seek position.
    player.pause_play(2).await.expect("pause");
    tokio::time::sleep(Duration::from_millis(200)).await;
    let reply = player
        .seek(Duration::from_secs(5), SeekDirection::Forward)
        .await
        .expect("seek reply");
    assert_eq!(reply.identifier, 2);
    assert!(
        reply.duration < Duration::from_secs(2),
        "failed seek must not move the bar to the phantom target, got {:?}",
        reply.duration
    );
}

#[tokio::test]
async fn seek_to_reports_pre_seek_position_on_failure() {

    let _guard = DEVICE_LOCK.lock().expect("device lock");
    if !device_available() {
        eprintln!("SKIP: no audio output device");
        return;
    }
    let player = AsyncRodio::<Unseekable, u32>::new();
    let _stream = player.play_song(Unseekable, 3);
    tokio::time::sleep(Duration::from_millis(300)).await;
    player.pause_play(3).await.expect("pause");
    tokio::time::sleep(Duration::from_millis(200)).await;
    let reply = player
        .seek_to(Duration::from_secs(60), 3)
        .await
        .expect("seek-to reply");
    assert_eq!(reply.identifier, 3);
    assert!(
        reply.duration < Duration::from_secs(2),
        "failed seek-to must not move the bar to the phantom target, got {:?}",
        reply.duration
    );
}

#[tokio::test]
async fn repeated_failed_seeks_do_not_accumulate_phantom() {
    let _guard = DEVICE_LOCK.lock().expect("device lock");
    if !device_available() {
        eprintln!("SKIP: no audio output device");
        return;
    }
    let player = AsyncRodio::<Unseekable, u32>::new();
    let _stream = player.play_song(Unseekable, 4);
    tokio::time::sleep(Duration::from_millis(300)).await;
    // Key repeat fires seeks back to back. A failed seek must not poison the
    // baseline of the next one, or the bar runs away while audio stays put.
    for _ in 0..8 {
        let reply = player
            .seek(Duration::from_secs(5), SeekDirection::Forward)
            .await
            .expect("seek reply");
        assert_eq!(reply.identifier, 4);
        assert!(
            reply.duration < Duration::from_secs(2),
            "repeated failed seeks must stay at audio position, got {:?}",
            reply.duration
        );
    }
}
