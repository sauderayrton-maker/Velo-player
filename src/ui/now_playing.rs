use std::cell::Cell;

use async_channel::Sender;
use gtk4::prelude::*;

use crate::spotify::{Command, PlaybackStatus, Track};

use super::cover_art;
use super::format_duration;

/// Left-hand "Now Playing" panel: a spinning vinyl record showing the
/// current track's album art, transport controls, a seek bar and a volume
/// slider.
pub struct NowPlayingPanel {
    pub root: gtk4::Box,
    record: gtk4::Overlay,
    album_art: gtk4::Picture,
    title_label: gtk4::Label,
    artist_label: gtk4::Label,
    album_label: gtk4::Label,
    play_pause_btn: gtk4::Button,
    progress_scale: gtk4::Scale,
    elapsed_label: gtk4::Label,
    duration_label: gtk4::Label,
    volume_scale: gtk4::Scale,
    duration_ms: Cell<u32>,
}

impl NowPlayingPanel {
    pub fn build(cmd_tx: Sender<Command>) -> Self {
        let root = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Vertical)
            .css_classes(vec!["now-playing-panel"])
            .width_request(340)
            .spacing(18)
            .build();

        // ── The record itself: disc, album art "label", and centre spindle ──
        let record = gtk4::Overlay::builder().css_classes(vec!["vinyl-record"]).build();

        let disc = gtk4::Box::builder()
            .css_classes(vec!["record-disc"])
            .halign(gtk4::Align::Center)
            .valign(gtk4::Align::Center)
            .width_request(280)
            .height_request(280)
            .build();
        record.set_child(Some(&disc));

        let album_art = gtk4::Picture::builder()
            .css_classes(vec!["record-label"])
            .width_request(132)
            .height_request(132)
            .content_fit(gtk4::ContentFit::Cover)
            .can_shrink(true)
            .halign(gtk4::Align::Center)
            .valign(gtk4::Align::Center)
            .build();
        album_art.set_overflow(gtk4::Overflow::Hidden);
        record.add_overlay(&album_art);

        let spindle = gtk4::Box::builder()
            .css_classes(vec!["record-spindle"])
            .halign(gtk4::Align::Center)
            .valign(gtk4::Align::Center)
            .width_request(14)
            .height_request(14)
            .build();
        record.add_overlay(&spindle);

        let record_frame = gtk4::Box::builder()
            .halign(gtk4::Align::Center)
            .margin_top(28)
            .build();
        record_frame.append(&record);

        // ── Track info ──
        let title_label = gtk4::Label::builder()
            .css_classes(vec!["now-playing-title"])
            .label("Nothing playing")
            .wrap(true)
            .justify(gtk4::Justification::Center)
            .lines(2)
            .ellipsize(gtk4::pango::EllipsizeMode::End)
            .build();
        let artist_label = gtk4::Label::builder()
            .css_classes(vec!["now-playing-artist"])
            .label("Connect to Spotify and pick something to play")
            .wrap(true)
            .justify(gtk4::Justification::Center)
            .lines(2)
            .ellipsize(gtk4::pango::EllipsizeMode::End)
            .build();
        let album_label = gtk4::Label::builder()
            .css_classes(vec!["now-playing-album"])
            .label("")
            .ellipsize(gtk4::pango::EllipsizeMode::End)
            .build();

        let info_box = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Vertical)
            .spacing(4)
            .margin_start(20)
            .margin_end(20)
            .build();
        info_box.append(&title_label);
        info_box.append(&artist_label);
        info_box.append(&album_label);

        // ── Seek bar ──
        let progress_scale = gtk4::Scale::with_range(gtk4::Orientation::Horizontal, 0.0, 1.0, 1.0);
        progress_scale.set_draw_value(false);
        progress_scale.add_css_class("progress-scale");
        progress_scale.set_hexpand(true);
        progress_scale.set_sensitive(false);

        let elapsed_label = gtk4::Label::builder().css_classes(vec!["time-label"]).label("0:00").build();
        let duration_label = gtk4::Label::builder().css_classes(vec!["time-label"]).label("0:00").build();

        let progress_row = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Horizontal)
            .spacing(8)
            .margin_start(20)
            .margin_end(20)
            .build();
        progress_row.append(&elapsed_label);
        progress_row.append(&progress_scale);
        progress_row.append(&duration_label);

        progress_scale.connect_change_value(glib::clone!(
            #[strong]
            cmd_tx,
            move |_, _, value| {
                let _ = cmd_tx.send_blocking(Command::Seek(value.max(0.0) as u32));
                glib::Propagation::Proceed
            }
        ));

        // ── Transport controls ──
        let prev_btn = gtk4::Button::from_icon_name("media-skip-backward-symbolic");
        prev_btn.add_css_class("transport-btn");
        let play_pause_btn = gtk4::Button::from_icon_name("media-playback-start-symbolic");
        play_pause_btn.add_css_class("transport-btn");
        play_pause_btn.add_css_class("transport-btn-primary");
        let next_btn = gtk4::Button::from_icon_name("media-skip-forward-symbolic");
        next_btn.add_css_class("transport-btn");

        prev_btn.connect_clicked(glib::clone!(
            #[strong]
            cmd_tx,
            move |_| {
                let _ = cmd_tx.send_blocking(Command::Previous);
            }
        ));
        play_pause_btn.connect_clicked(glib::clone!(
            #[strong]
            cmd_tx,
            move |_| {
                let _ = cmd_tx.send_blocking(Command::TogglePlayPause);
            }
        ));
        next_btn.connect_clicked(glib::clone!(
            #[strong]
            cmd_tx,
            move |_| {
                let _ = cmd_tx.send_blocking(Command::Next);
            }
        ));

        let controls_row = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Horizontal)
            .css_classes(vec!["transport-controls"])
            .halign(gtk4::Align::Center)
            .spacing(18)
            .build();
        controls_row.append(&prev_btn);
        controls_row.append(&play_pause_btn);
        controls_row.append(&next_btn);

        // ── Volume ──
        let volume_icon = gtk4::Image::from_icon_name("audio-volume-high-symbolic");
        let volume_scale = gtk4::Scale::with_range(gtk4::Orientation::Horizontal, 0.0, 100.0, 1.0);
        volume_scale.set_draw_value(false);
        volume_scale.set_value(50.0);
        volume_scale.set_hexpand(true);
        volume_scale.add_css_class("volume-scale");

        volume_scale.connect_change_value(glib::clone!(
            #[strong]
            cmd_tx,
            move |_, _, value| {
                let _ = cmd_tx.send_blocking(Command::SetVolume((value / 100.0).clamp(0.0, 1.0)));
                glib::Propagation::Proceed
            }
        ));

        let volume_row = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Horizontal)
            .css_classes(vec!["volume-row"])
            .spacing(8)
            .margin_start(20)
            .margin_end(20)
            .margin_bottom(20)
            .valign(gtk4::Align::End)
            .vexpand(true)
            .build();
        volume_row.append(&volume_icon);
        volume_row.append(&volume_scale);

        root.append(&record_frame);
        root.append(&info_box);
        root.append(&progress_row);
        root.append(&controls_row);
        root.append(&volume_row);

        Self {
            root,
            record,
            album_art,
            title_label,
            artist_label,
            album_label,
            play_pause_btn,
            progress_scale,
            elapsed_label,
            duration_label,
            volume_scale,
            duration_ms: Cell::new(0),
        }
    }

    /// Updates the track title/artist/album, album art, and seek bar range
    /// for the newly loaded track (or resets to the empty state if `None`).
    pub fn set_now_playing(&self, track: Option<&Track>) {
        match track {
            Some(track) => {
                self.title_label.set_label(&track.name);
                self.artist_label.set_label(&track.artist_names());
                self.album_label.set_label(&track.album);
                self.duration_ms.set(track.duration_ms);
                self.progress_scale.set_range(0.0, track.duration_ms.max(1) as f64);
                self.progress_scale.set_sensitive(true);
                self.duration_label.set_label(&format_duration(track.duration_ms));
                cover_art::set_picture_from_url(&self.album_art, track.album_art.clone());
            }
            None => {
                self.title_label.set_label("Nothing playing");
                self.artist_label.set_label("Pick a track from a playlist or search");
                self.album_label.set_label("");
                self.duration_ms.set(0);
                self.progress_scale.set_range(0.0, 1.0);
                self.progress_scale.set_value(0.0);
                self.progress_scale.set_sensitive(false);
                self.elapsed_label.set_label("0:00");
                self.duration_label.set_label("0:00");
                cover_art::set_picture_from_url(&self.album_art, None);
            }
        }
    }

    /// Toggles the play/pause icon and starts or stops the record's spin.
    pub fn set_playback_status(&self, status: PlaybackStatus) {
        let icon = match status {
            PlaybackStatus::Playing => "media-playback-pause-symbolic",
            PlaybackStatus::Paused | PlaybackStatus::Stopped => "media-playback-start-symbolic",
        };
        self.play_pause_btn.set_icon_name(icon);

        if status == PlaybackStatus::Playing {
            self.record.add_css_class("spinning");
        } else {
            self.record.remove_css_class("spinning");
        }
    }

    /// Reflects the mixer's current volume (`0..=u16::MAX`) on the slider.
    pub fn set_volume(&self, volume: u16) {
        self.volume_scale.set_value(f64::from(volume) / f64::from(u16::MAX) * 100.0);
    }

    /// Updates the seek bar position and elapsed/duration labels.
    pub fn set_position(&self, position_ms: u32, duration_ms: u32) {
        if duration_ms > 0 && duration_ms != self.duration_ms.get() {
            self.duration_ms.set(duration_ms);
            self.progress_scale.set_range(0.0, duration_ms as f64);
            self.duration_label.set_label(&format_duration(duration_ms));
        }
        self.progress_scale.set_value(position_ms as f64);
        self.elapsed_label.set_label(&format_duration(position_ms));
    }
}
