use std::cell::RefCell;
use std::rc::Rc;

use async_channel::Sender;
use gtk4::prelude::*;

use crate::spotify::{Command, Track};

use super::cover_art;

/// Right-hand sidebar listing recently played tracks. Clicking one replays
/// the recents list starting from that track.
pub struct RecentsPanel {
    pub root: gtk4::Box,
    list_box: gtk4::ListBox,
    stack: gtk4::Stack,
    tracks: Rc<RefCell<Vec<Track>>>,
    now_playing_uri: Rc<RefCell<Option<String>>>,
}

impl RecentsPanel {
    pub fn build(cmd_tx: Sender<Command>) -> Self {
        let root = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Vertical)
            .css_classes(vec!["velo-sidebar", "recents-panel"])
            .width_request(300)
            .build();

        let header = gtk4::Box::builder().css_classes(vec!["sidebar-header"]).build();
        let title = gtk4::Label::builder()
            .css_classes(vec!["panel-title"])
            .label("RECENTS")
            .halign(gtk4::Align::Start)
            .hexpand(true)
            .valign(gtk4::Align::Center)
            .build();
        header.append(&title);

        let list_box = gtk4::ListBox::builder()
            .css_classes(vec!["panel-list"])
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
            .icon_name("document-open-recent-symbolic")
            .title("Nothing played yet")
            .description("Tracks you play will show up here.")
            .vexpand(true)
            .build();

        let stack = gtk4::Stack::new();
        stack.add_named(&empty_page, Some("empty"));
        stack.add_named(&scroller, Some("recents"));
        stack.set_visible_child_name("empty");

        root.append(&header);
        root.append(&stack);

        Self { root, list_box, stack, tracks, now_playing_uri: Rc::new(RefCell::new(None)) }
    }

    /// Replaces the recents list with `tracks` (most-recent-first).
    pub fn set_recents(&self, tracks: Vec<Track>) {
        while let Some(child) = self.list_box.first_child() {
            self.list_box.remove(&child);
        }

        if tracks.is_empty() {
            *self.tracks.borrow_mut() = tracks;
            self.stack.set_visible_child_name("empty");
            return;
        }

        let now_playing = self.now_playing_uri.borrow().clone();
        for track in &tracks {
            let row = build_track_row(track);
            if Some(&track.uri) == now_playing.as_ref() {
                row.add_css_class("playing");
            }
            self.list_box.append(&row);
        }

        *self.tracks.borrow_mut() = tracks;
        self.stack.set_visible_child_name("recents");
    }

    /// Highlights the row matching `uri` (if it's part of the recents list)
    /// and un-highlights everything else.
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
        .css_classes(vec!["playlist-art"])
        .width_request(40)
        .height_request(40)
        .content_fit(gtk4::ContentFit::Cover)
        .can_shrink(true)
        .build();
    art.set_overflow(gtk4::Overflow::Hidden);
    cover_art::set_picture_from_url(&art, track.album_art.clone());

    let title = gtk4::Label::builder()
        .css_classes(vec!["row-title"])
        .label(&track.name)
        .halign(gtk4::Align::Start)
        .ellipsize(gtk4::pango::EllipsizeMode::End)
        .build();
    let subtitle = gtk4::Label::builder()
        .css_classes(vec!["row-meta"])
        .label(track.artist_names())
        .halign(gtk4::Align::Start)
        .ellipsize(gtk4::pango::EllipsizeMode::End)
        .build();

    let labels = gtk4::Box::builder().orientation(gtk4::Orientation::Vertical).valign(gtk4::Align::Center).hexpand(true).build();
    labels.append(&title);
    labels.append(&subtitle);

    let row_box = gtk4::Box::builder().orientation(gtk4::Orientation::Horizontal).css_classes(vec!["playlist-row"]).spacing(12).build();
    row_box.append(&art);
    row_box.append(&labels);

    let row = gtk4::ListBoxRow::builder().child(&row_box).build();
    row.set_focusable(true);
    row
}
