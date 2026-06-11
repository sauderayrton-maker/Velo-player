use std::cell::RefCell;
use std::rc::Rc;

use async_channel::Sender;
use gtk4::prelude::*;

use crate::spotify::{Command, Track};

use super::cover_art;
use super::format_duration;

/// The centre content area: a header (playlist name / "Search results"),
/// and a scrollable list of tracks. Used for both search results and
/// playlist browsing. Clicking a row replaces the play queue with the
/// currently shown tracks, starting at the clicked one.
pub struct TrackListPanel {
    pub root: gtk4::Box,
    stack: gtk4::Stack,
    header_label: gtk4::Label,
    list_box: gtk4::ListBox,
    tracks: Rc<RefCell<Vec<Track>>>,
    now_playing_uri: Rc<RefCell<Option<String>>>,
}

impl TrackListPanel {
    pub fn build(cmd_tx: Sender<Command>) -> Self {
        let root = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Vertical)
            .css_classes(vec!["track-list-panel"])
            .hexpand(true)
            .build();

        let header_label = gtk4::Label::builder()
            .css_classes(vec!["content-header"])
            .halign(gtk4::Align::Start)
            .ellipsize(gtk4::pango::EllipsizeMode::End)
            .build();
        header_label.set_visible(false);

        let list_box = gtk4::ListBox::builder()
            .css_classes(vec!["track-list"])
            .selection_mode(gtk4::SelectionMode::None)
            .build();

        let tracks: Rc<RefCell<Vec<Track>>> = Rc::new(RefCell::new(Vec::new()));

        list_box.connect_row_activated(glib::clone!(
            #[strong]
            cmd_tx,
            #[strong]
            tracks,
            move |_, row| {
                let index = row.index();
                if index < 0 {
                    return;
                }
                let queue = tracks.borrow().clone();
                let _ = cmd_tx.send_blocking(Command::PlayQueue { tracks: queue, start_index: index as usize });
            }
        ));

        let scroller = gtk4::ScrolledWindow::builder()
            .hscrollbar_policy(gtk4::PolicyType::Never)
            .vexpand(true)
            .child(&list_box)
            .build();

        let empty_page = libadwaita::StatusPage::builder()
            .icon_name("folder-music-symbolic")
            .title("Nothing to show yet")
            .description("Search for a song, or pick a playlist on the right.")
            .vexpand(true)
            .build();

        let stack = gtk4::Stack::new();
        stack.add_named(&empty_page, Some("empty"));
        stack.add_named(&scroller, Some("tracks"));
        stack.set_visible_child_name("empty");

        root.append(&header_label);
        root.append(&stack);

        Self {
            root,
            stack,
            header_label,
            list_box,
            tracks,
            now_playing_uri: Rc::new(RefCell::new(None)),
        }
    }

    /// Replaces the visible list with `tracks`, labelled by `header`.
    pub fn show_tracks(&self, header: &str, tracks: Vec<Track>) {
        while let Some(child) = self.list_box.first_child() {
            self.list_box.remove(&child);
        }

        if tracks.is_empty() {
            self.header_label.set_visible(false);
            *self.tracks.borrow_mut() = tracks;
            self.stack.set_visible_child_name("empty");
            return;
        }

        self.header_label.set_label(header);
        self.header_label.set_visible(true);

        let now_playing = self.now_playing_uri.borrow().clone();
        for track in &tracks {
            let row = build_track_row(track);
            if Some(&track.uri) == now_playing.as_ref() {
                row.add_css_class("playing");
            }
            self.list_box.append(&row);
        }

        *self.tracks.borrow_mut() = tracks;
        self.stack.set_visible_child_name("tracks");
    }

    /// Highlights the row matching `uri` (if it's part of the currently
    /// shown list) and un-highlights everything else.
    pub fn set_now_playing_uri(&self, uri: Option<String>) {
        *self.now_playing_uri.borrow_mut() = uri.clone();

        let tracks = self.tracks.borrow();
        let mut index = 0;
        while let Some(row) = self.list_box.row_at_index(index) {
            let is_playing = tracks.get(index as usize).map(|t| Some(&t.uri) == uri.as_ref()).unwrap_or(false);
            if is_playing {
                row.add_css_class("playing");
            } else {
                row.remove_css_class("playing");
            }
            index += 1;
        }
    }
}

fn build_track_row(track: &Track) -> gtk4::ListBoxRow {
    let art = gtk4::Picture::builder()
        .css_classes(vec!["track-art"])
        .width_request(44)
        .height_request(44)
        .content_fit(gtk4::ContentFit::Cover)
        .can_shrink(true)
        .build();
    art.set_overflow(gtk4::Overflow::Hidden);
    cover_art::set_picture_from_url(&art, track.album_art.clone());

    let title = gtk4::Label::builder()
        .css_classes(vec!["track-title"])
        .label(&track.name)
        .halign(gtk4::Align::Start)
        .ellipsize(gtk4::pango::EllipsizeMode::End)
        .build();
    let subtitle = gtk4::Label::builder()
        .css_classes(vec!["track-subtitle"])
        .label(format!("{} — {}", track.artist_names(), track.album))
        .halign(gtk4::Align::Start)
        .ellipsize(gtk4::pango::EllipsizeMode::End)
        .build();

    let labels = gtk4::Box::builder().orientation(gtk4::Orientation::Vertical).valign(gtk4::Align::Center).hexpand(true).build();
    labels.append(&title);
    labels.append(&subtitle);

    let duration = gtk4::Label::builder()
        .css_classes(vec!["track-duration"])
        .label(format_duration(track.duration_ms))
        .valign(gtk4::Align::Center)
        .build();

    let row_box = gtk4::Box::builder().orientation(gtk4::Orientation::Horizontal).css_classes(vec!["track-row"]).spacing(12).build();
    row_box.append(&art);
    row_box.append(&labels);
    row_box.append(&duration);

    let row = gtk4::ListBoxRow::builder().child(&row_box).build();
    row.set_focusable(true);
    row
}
