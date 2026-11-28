use gpui::{
    App, Application, Bounds, Context, Div, FocusHandle, KeyBinding, PathPromptOptions,
    ScrollStrategy, Stateful, Task, TitlebarOptions, UniformListScrollHandle, Window, WindowBounds,
    WindowOptions, actions, div, prelude::*, px, rgb, size, uniform_list,
};
use retroamp::audio::{AudioEngine, PlaybackState};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

actions!(
    retroamp_gpui,
    [
        OpenAudio,
        TogglePlayback,
        PlayTrack,
        PreviousTrack,
        StopTrack,
        NextTrack,
        VolumeUp,
        VolumeDown,
        FocusNext,
        FocusPrevious
    ]
);

const INK: u32 = 0xe8f1e7;
const MUTED: u32 = 0xb0c4b8;
const PHOSPHOR: u32 = 0xb7f99c;
const PANEL: u32 = 0x1e2a28;
const LCD: u32 = 0x111b18;
const FRAME_INTERVAL: Duration = Duration::from_millis(33);
const SURFACE: u32 = 0x293832;
const SURFACE_HOVER: u32 = 0x3c5144;
const SURFACE_PRESS: u32 = 0x1b2822;
const ACCENT_TEXT: u32 = 0x172218;

#[derive(Default)]
struct Queue {
    files: Vec<PathBuf>,
    current: Option<usize>,
}

impl Queue {
    fn select(&mut self, index: usize) -> Option<&Path> {
        let path = self.files.get(index)?;
        self.current = Some(index);
        Some(path)
    }

    fn neighbor(&self, offset: isize) -> Option<usize> {
        self.current?
            .checked_add_signed(offset)
            .filter(|&i| i < self.files.len())
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Control {
    Open,
    Previous,
    PlayPause,
    Stop,
    Next,
}

struct Player {
    audio: AudioEngine,
    queue: Queue,
    scroll: UniformListScrollHandle,
    focus: FocusHandle,
    ticks: Option<Task<()>>,
    picking: bool,
    loading: bool,
    load_generation: u64,
    load_cancel: Option<Arc<AtomicBool>>,
    error: Option<String>,
    /// Tracks one auto-advance chain: the titles it skipped and the row the
    /// chain started from, so a fully failed chain reports itself and puts
    /// the selection back instead of leaving it on the last broken entry.
    skipped: Vec<String>,
    chain_origin: Option<usize>,
}

/// One auto-advance chain that ran out of playable tracks. A single failure
/// reads like the old message; several failures list everything it tried.
fn chain_failure_message(skipped: &[String], error: &str) -> String {
    match skipped {
        [only] => format!("Can't play {only}: {error}"),
        names => format!("Skipped unplayable tracks ({}): {error}", names.join(", ")),
    }
}

impl Player {
    fn new(files: Vec<PathBuf>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let focus = cx.focus_handle();
        focus.focus(window);
        let mut player = Self {
            audio: AudioEngine::new(),
            queue: Queue {
                files,
                current: None,
            },
            scroll: UniformListScrollHandle::new(),
            focus,
            ticks: None,
            picking: false,
            loading: false,
            load_generation: 0,
            load_cancel: None,
            error: None,
            skipped: Vec::new(),
            chain_origin: None,
        };
        if !player.queue.files.is_empty() {
            player.start(0, cx, false);
        }
        player
    }

    fn invalidate_load(&mut self) {
        self.load_generation = self.load_generation.wrapping_add(1);
        if let Some(cancel) = self.load_cancel.take() {
            cancel.store(true, Ordering::SeqCst);
        }
        self.loading = false;
    }

    fn start(&mut self, index: usize, cx: &mut Context<Self>, auto_advance: bool) {
        // A manual start abandons any chain in progress; an auto-advance that
        // begins one remembers where it started so a fully failed chain can
        // restore the selection there.
        if auto_advance {
            self.chain_origin.get_or_insert(self.queue.current.unwrap_or(index));
        } else {
            self.skipped.clear();
            self.chain_origin = None;
        }

        let Some(path) = self.queue.select(index).map(Path::to_path_buf) else {
            return;
        };

        self.invalidate_load();
        let generation = self.load_generation;
        let cancellation = Arc::new(AtomicBool::new(false));
        let worker_cancellation = cancellation.clone();
        let mut loading_audio = std::mem::replace(&mut self.audio, AudioEngine::new());
        let mut standby_audio = AudioEngine::new();
        standby_audio.set_volume(loading_audio.volume);
        standby_audio.set_balance(loading_audio.balance);
        standby_audio.update_eq(loading_audio.eq);
        self.audio = standby_audio;
        self.loading = true;
        self.load_cancel = Some(cancellation);
        self.error = None;
        self.scroll.scroll_to_item(index, ScrollStrategy::Center);
        cx.notify();

        let worker_path = path.clone();
        let background = cx.background_executor().spawn(async move {
            let result =
                loading_audio.play_file_with_cancellation(&worker_path, &worker_cancellation);
            (loading_audio, result)
        });

        cx.spawn(async move |this, cx| {
            let (mut loaded_audio, result) = background.await;
            let _ = this.update(cx, move |player, cx| {
                if player.load_generation != generation {
                    return;
                }
                player.loading = false;
                player.load_cancel = None;

                match result {
                    Ok(()) => {
                        loaded_audio.set_volume(player.audio.volume);
                        loaded_audio.set_balance(player.audio.balance);
                        loaded_audio.update_eq(player.audio.eq);
                        player.audio = loaded_audio;
                        player.error = if player.skipped.is_empty() {
                            None
                        } else {
                            Some(format!(
                                "Skipped unplayable tracks: {}",
                                player.skipped.join(", ")
                            ))
                        };
                        player.skipped.clear();
                        player.chain_origin = None;
                    }
                    Err(error) => {
                        player.skipped.push(file_name(&path));
                        if auto_advance && let Some(next) = player.queue.neighbor(1) {
                            player.start(next, cx, true);
                        } else {
                            // The chain is out of tracks: report every skipped
                            // title and put the selection back where the chain
                            // began, so Play retries the origin, not the last
                            // broken entry.
                            player.error =
                                Some(chain_failure_message(&player.skipped, &error));
                            if let Some(origin) = player.chain_origin {
                                player.queue.current = Some(origin);
                                player
                                    .scroll
                                    .scroll_to_item(origin, ScrollStrategy::Center);
                            }
                            player.skipped.clear();
                            player.chain_origin = None;
                        }
                    }
                }

                player.sync_ticks(cx);
                cx.notify();
            });
        })
        .detach();
    }

    // Poll EOF independently of drawing so the queue advances in a minimized window.
    // Dropping the task stops the timer completely when paused or stopped.
    fn sync_ticks(&mut self, cx: &mut Context<Self>) {
        self.ticks = self.audio.is_playing().then(|| {
            cx.spawn(async |this, cx| {
                loop {
                    cx.background_executor().timer(FRAME_INTERVAL).await;
                    let playing = this.update(cx, |player, cx| {
                        if player.audio.update()
                            && let Some(next) = player.queue.neighbor(1)
                        {
                            player.start(next, cx, true);
                        }
                        cx.notify();
                        player.audio.is_playing()
                    });
                    if !matches!(playing, Ok(true)) {
                        break;
                    }
                }
            })
        });
    }

    fn open(&mut self, cx: &mut Context<Self>) {
        if self.picking {
            return;
        }
        self.picking = true;
        cx.notify();
        let selection = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: true,
            prompt: Some("Open audio".into()),
        });
        cx.spawn(async move |this, cx| {
            let selection = selection.await;
            let _ = this.update(cx, |player, cx| {
                player.picking = false;
                match selection {
                    Ok(Ok(Some(files))) if !files.is_empty() => {
                        player.queue = Queue {
                            files,
                            current: None,
                        };
                        player.start(0, cx, false);
                    }
                    Ok(Err(e)) => player.error = Some(format!("Can't open file picker: {e}")),
                    Err(_) => {
                        player.error =
                            Some("File picker unavailable. Pass paths on the command line.".into())
                    }
                    _ => {} // Cancelling leaves playback and the queue untouched.
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn play(&mut self, cx: &mut Context<Self>) {
        if self.loading {
            return;
        }
        match self.audio.state {
            PlaybackState::Playing => return,
            PlaybackState::Paused => self.audio.resume(),
            PlaybackState::Stopped => match self.queue.current {
                Some(i) => self.start(i, cx, false),
                None => return self.open(cx),
            },
        }
        self.sync_ticks(cx);
        cx.notify();
    }

    fn control(&mut self, control: Control, cx: &mut Context<Self>) {
        if self.loading && control == Control::PlayPause {
            return;
        }
        match control {
            Control::Open => return self.open(cx),
            Control::PlayPause => match self.audio.state {
                PlaybackState::Playing => self.audio.pause(),
                PlaybackState::Paused => self.audio.resume(),
                PlaybackState::Stopped => match self.queue.current {
                    Some(i) => self.start(i, cx, false),
                    None => return self.open(cx),
                },
            },
            Control::Stop => {
                self.invalidate_load();
                self.audio.stop();
            }
            Control::Previous | Control::Next => {
                let offset = if matches!(control, Control::Previous) {
                    -1
                } else {
                    1
                };
                if let Some(i) = self.queue.neighbor(offset) {
                    self.start(i, cx, false);
                }
            }
        }
        self.sync_ticks(cx);
        cx.notify();
    }

    fn button(
        &self,
        id: &'static str,
        label: &'static str,
        command: Control,
        enabled: bool,
        primary: bool,
        cx: &Context<Self>,
    ) -> Stateful<Div> {
        let primary = enabled && primary;
        let fill = if primary { PHOSPHOR } else { SURFACE };
        let text = if primary { ACCENT_TEXT } else { INK };
        let hover = if primary { 0xc9ffb6 } else { SURFACE_HOVER };

        div()
            .id(id)
            .flex_1()
            .h(px(40.))
            .flex()
            .items_center()
            .justify_center()
            .bg(rgb(fill))
            .text_color(rgb(text))
            .border_1()
            .border_color(rgb(0x526657))
            .rounded_sm()
            .when(enabled, |b| {
                b.tab_index(0)
                    .cursor_pointer()
                    .focus(|b| {
                        b.bg(rgb(SURFACE_PRESS))
                            .text_color(rgb(PHOSPHOR))
                            .border_2()
                            .border_color(rgb(PHOSPHOR))
                    })
                    .hover(|b| b.bg(rgb(hover)))
                    .active(|b| b.bg(rgb(if primary { 0x82ad6e } else { SURFACE_PRESS })))
                    .on_click(cx.listener(move |this, _, _, cx| this.control(command, cx)))
            })
            .when(!enabled, |b| b.opacity(0.45))
            .child(label)
    }
}

impl Drop for Player {
    fn drop(&mut self) {
        if let Some(cancel) = &self.load_cancel {
            cancel.store(true, Ordering::SeqCst);
        }
    }
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .unwrap_or(path.as_os_str())
        .to_string_lossy()
        .into_owned()
}

fn clock(time: Duration) -> String {
    let secs = time.as_secs();
    format!("{:02}:{:02}", secs / 60, secs % 60)
}

impl Render for Player {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let selected = self.queue.current.and_then(|i| self.queue.files.get(i));
        let track = self
            .audio
            .current_track
            .as_ref()
            .filter(|t| selected == Some(&t.path));
        let title = track
            .map(|t| t.title.clone())
            .or_else(|| selected.map(|p| file_name(p)))
            .unwrap_or_else(|| "No track loaded".into());
        let (state, state_color) = if self.loading {
            ("LOADING", 0xffd28a)
        } else {
            match self.audio.state {
                PlaybackState::Playing => ("PLAYING", PHOSPHOR),
                PlaybackState::Paused => ("PAUSED", 0xffd28a),
                PlaybackState::Stopped => ("STOPPED", MUTED),
            }
        };
        let artist = track
            .map(|t| t.artist.clone())
            .filter(|artist| !artist.is_empty());
        let duration = track
            .map(|t| clock(t.duration))
            .unwrap_or_else(|| "--:--".into());
        // The engine plays the sink buffer out past the header duration, so
        // the shown elapsed time must never run past the end.
        let position = track
            .map(|t| self.audio.position.min(t.duration))
            .unwrap_or(self.audio.position);
        let play_label = if self.loading {
            "Loading…"
        } else {
            match self.audio.state {
                PlaybackState::Playing => "Pause",
                PlaybackState::Paused => "Resume",
                PlaybackState::Stopped => "Play",
            }
        };
        let (_, spectrum, _) = self.audio.get_vis_data();
        let count = self.queue.files.len();
        div()
            .track_focus(&self.focus)
            .size_full()
            .flex()
            .flex_col()
            .gap_2()
            .p_3()
            .bg(rgb(PANEL))
            .text_color(rgb(INK))
            .text_sm()
            .on_action(cx.listener(|this, _: &OpenAudio, _, cx| this.open(cx)))
            .on_action(
                cx.listener(|this, _: &TogglePlayback, _, cx| this.control(Control::PlayPause, cx)),
            )
            .on_action(cx.listener(|this, _: &PlayTrack, _, cx| this.play(cx)))
            .on_action(
                cx.listener(|this, _: &PreviousTrack, _, cx| this.control(Control::Previous, cx)),
            )
            .on_action(cx.listener(|this, _: &StopTrack, _, cx| this.control(Control::Stop, cx)))
            .on_action(cx.listener(|this, _: &NextTrack, _, cx| this.control(Control::Next, cx)))
            .on_action(cx.listener(|this, _: &VolumeUp, _, cx| {
                this.audio.set_volume(this.audio.volume + 0.05);
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &VolumeDown, _, cx| {
                this.audio.set_volume(this.audio.volume - 0.05);
                cx.notify();
            }))
            .on_action(|_: &FocusNext, window, _| window.focus_next())
            .on_action(|_: &FocusPrevious, window, _| window.focus_prev())
            .child(
                div()
                    .flex()
                    .justify_between()
                    .text_color(rgb(MUTED))
                    .child(div().font_family("monospace").child("RETROAMP"))
                    .child(format!(
                        "VOL {:02}%",
                        (self.audio.volume * 100.0).round() as u8
                    )),
            )
            .child(
                div()
                    .bg(rgb(LCD))
                    .p_3()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .overflow_hidden()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .child(div().text_color(rgb(state_color)).child(state))
                            .child(
                                div()
                                    .font_family("monospace")
                                    .child(format!("{} / {duration}", clock(position))),
                            ),
                    )
                    .child(div().text_lg().truncate().child(title))
                    .when_some(artist, |display, artist| {
                        display.child(div().text_color(rgb(MUTED)).child(artist))
                    })
                    .child(
                        div()
                            .h(px(29.))
                            .flex()
                            .items_end()
                            .gap(px(2.))
                            .children(spectrum.map(|level| {
                                div()
                                    .flex_1()
                                    .h(px(2. + 27. * level.clamp(0., 1.)))
                                    .bg(rgb(PHOSPHOR))
                            })),
                    ),
            )
            .child(
                div()
                    .flex()
                    .gap_1()
                    .child(self.button(
                        "open",
                        if self.picking {
                            "Opening…"
                        } else {
                            "Open…"
                        },
                        Control::Open,
                        !self.picking,
                        count == 0,
                        cx,
                    ))
                    .child(self.button(
                        "prev",
                        "Previous",
                        Control::Previous,
                        self.queue.neighbor(-1).is_some(),
                        false,
                        cx,
                    ))
                    .child(self.button(
                        "play",
                        play_label,
                        Control::PlayPause,
                        count > 0 && !self.loading,
                        true,
                        cx,
                    ))
                    .child(self.button(
                        "stop",
                        "Stop",
                        Control::Stop,
                        self.loading || self.audio.state != PlaybackState::Stopped,
                        false,
                        cx,
                    ))
                    .child(self.button(
                        "next",
                        "Next",
                        Control::Next,
                        self.queue.neighbor(1).is_some(),
                        false,
                        cx,
                    )),
            )
            .when_some(self.error.clone(), |d, e| {
                d.child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_1()
                        // A long reason must not grow this block into the playlist's space.
                        .child(div().text_color(rgb(0xffc8a8)).line_clamp(2).child(e))
                        .child(
                            div()
                                .text_color(rgb(MUTED))
                                .child("Select another track or open different files."),
                        ),
                )
            })
            .child(div().text_color(rgb(MUTED)).child(format!(
                "PLAYLIST   /   {count} {}",
                if count == 1 { "track" } else { "tracks" }
            )))
            .when(count == 0, |d| {
                d.child(
                    div()
                        .p_2()
                        .text_color(rgb(MUTED))
                        .child("Open audio files to start a playlist.  Ctrl+O"),
                )
            })
            .when(count > 0, |d| {
                d.child(
                    uniform_list(
                        "playlist",
                        count,
                        cx.processor(|this, range: std::ops::Range<usize>, _, cx| {
                            range
                                .map(|i| {
                                    let current = this.queue.current == Some(i);
                                    div()
                                        .id(("track", i))
                                        .h(px(32.))
                                        .px_2()
                                        .flex()
                                        .gap_2()
                                        .items_center()
                                        .tab_index(0)
                                        .cursor_pointer()
                                        .border_1()
                                        .border_color(rgb(LCD))
                                        .bg(rgb(if current { SURFACE } else { LCD }))
                                        .focus(|r| r.border_2().border_color(rgb(PHOSPHOR)))
                                        .hover(|r| r.bg(rgb(SURFACE_HOVER)))
                                        .active(|r| r.bg(rgb(SURFACE_PRESS)))
                                        .child(
                                            div()
                                                .text_color(rgb(MUTED))
                                                .child(format!("{:02}", i + 1)),
                                        )
                                        .child(
                                            div().truncate().child(file_name(&this.queue.files[i])),
                                        )
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            this.start(i, cx, false);
                                        }))
                                })
                                .collect()
                        }),
                    )
                    .track_scroll(self.scroll.clone())
                    .flex_1()
                    .min_h_0(),
                )
            })
            .child(div().text_xs().text_color(rgb(MUTED)).child(
                "L Open · Z Previous · X Play · C Pause/Resume · V Stop · B Next · ↑/↓ Volume",
            ))
    }
}

fn main() {
    env_logger::init();
    let files = std::env::args_os().skip(1).map(PathBuf::from).collect();
    Application::new().run(move |cx: &mut App| {
        cx.bind_keys([
            KeyBinding::new("ctrl-o", OpenAudio, None),
            KeyBinding::new("l", OpenAudio, None),
            KeyBinding::new("z", PreviousTrack, None),
            KeyBinding::new("x", PlayTrack, None),
            KeyBinding::new("c", TogglePlayback, None),
            KeyBinding::new("v", StopTrack, None),
            KeyBinding::new("b", NextTrack, None),
            KeyBinding::new("up", VolumeUp, None),
            KeyBinding::new("down", VolumeDown, None),
            KeyBinding::new("tab", FocusNext, None),
            KeyBinding::new("shift-tab", FocusPrevious, None),
        ]);
        cx.on_window_closed(|cx| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();
        // Header, LCD panel, transport, playlist heading and key hints take about 305 px,
        // plus about 52 px while an error is shown. Rows are 32 px, so these heights keep
        // roughly 5 rows visible at the default size and at least 2 at the minimum.
        let bounds = Bounds::centered(None, size(px(460.), px(480.)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                window_min_size: Some(size(px(380.), px(440.))),
                titlebar: Some(TitlebarOptions {
                    title: Some("RetroAmp · GPUI".into()),
                    ..Default::default()
                }),
                app_id: Some("retroamp-gpui".into()),
                ..Default::default()
            },
            move |window, cx| cx.new(|cx| Player::new(files, window, cx)),
        )
        .expect("could not open GPUI window");
        cx.activate(true);
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn navigation_never_wraps_or_selects_out_of_range() {
        let mut queue = Queue::default();
        assert_eq!(queue.neighbor(1), None);
        queue.files = vec!["one.wav".into(), "two.wav".into()];
        assert_eq!(queue.select(0), Some(Path::new("one.wav")));
        assert_eq!(queue.neighbor(-1), None);
        assert_eq!(queue.neighbor(1), Some(1));
        queue.select(1);
        assert_eq!(queue.neighbor(1), None);
        assert!(queue.select(2).is_none());
        assert_eq!(queue.current, Some(1));
        assert_eq!(queue.neighbor(-1), Some(0));
    }

    #[test]
    fn a_single_chain_failure_names_the_track_and_reason() {
        let message = chain_failure_message(&["a.wav".into()], "the decoder rejected the file");
        assert_eq!(message, "Can't play a.wav: the decoder rejected the file");
    }

    #[test]
    fn an_exhausted_chain_lists_every_skipped_title() {
        let message = chain_failure_message(
            &["a.wav".into(), "b.wav".into(), "c.wav".into()],
            "No audio output device available",
        );
        assert!(message.starts_with("Skipped unplayable tracks (a.wav, b.wav, c.wav):"), "{message}");
        assert!(message.ends_with("No audio output device available"));
    }
}
