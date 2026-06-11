use std::cell::RefCell;
use std::rc::Rc;

use async_channel::Sender;
use gtk4::prelude::*;

use crate::spotify::{Command, Playlist};

use super::cover_art;

/// Right-hand sidebar listing the current user's playlists. Clicking one
/// asks the controller to fetch its tracks.
pub struct PlaylistsPanel {
    pub root: gtk4::Box,
    list_box: gtk4::ListBox,
    stack: gtk4::Stack,
    playlists: Rc<RefCell<Vec<Playlist>>>,
}

impl PlaylistsPanel {
    pub fn build(cmd_tx: Sender<Command>) -> Self {
        let root = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Vertical)
            .css_classes(vec!["velo-sidebar", "playlists-panel"])
            .width_request(300)
            .build();

        let header = gtk4::Box::builder().css_classes(vec!["sidebar-header"]).build();
        let title = gtk4::Label::builder()
            .css_classes(vec!["panel-title"])
            .label("PLAYLISTS")
            .halign(gtk4::Align::Start)
            .hexpand(true)
            .build();
        header.append(&title);

        let refresh_btn = gtk4::Button::builder()
            .icon_name("view-refresh-symbolic")
            .tooltip_text("Refresh playlists")
            .css_classes(vec!["flat", "sidebar-icon-btn"])
            .valign(gtk4::Align::Center)
            .build();
        refresh_btn.connect_clicked(glib::clone!(
            #[strong]
            cmd_tx,
            move |_| {
                let _ = cmd_tx.send_blocking(Command::LoadPlaylists);
            }
        ));
        header.append(&refresh_btn);

        let list_box = gtk4::ListBox::builder()
            .css_classes(vec!["panel-list"])
            .selection_mode(gtk4::SelectionMode::None)
            .build();

        let playlists: Rc<RefCell<Vec<Playlist>>> = Rc::new(RefCell::new(Vec::new()));

        list_box.connect_row_activated(glib::clone!(
            #[strong]
            cmd_tx,
            #[strong]
            playlists,
            move |_, row| {
                let index = row.index();
                if index < 0 {
                    return;
                }
                if let Some(playlist) = playlists.borrow().get(index as usize) {
                    let _ = cmd_tx.send_blocking(Command::LoadPlaylist(playlist.clone()));
                }
            }
        ));

        let scroller = gtk4::ScrolledWindow::builder()
            .hscrollbar_policy(gtk4::PolicyType::Never)
            .vexpand(true)
            .child(&list_box)
            .build();

        let empty_page = libadwaita::StatusPage::builder()
            .icon_name("view-list-symbolic")
            .title("No playlists yet")
            .description("Connect to Spotify to see your library here.")
            .vexpand(true)
            .build();

        let stack = gtk4::Stack::new();
        stack.add_named(&empty_page, Some("empty"));
        stack.add_named(&scroller, Some("playlists"));
        stack.set_visible_child_name("empty");

        root.append(&header);
        root.append(&stack);

        Self { root, list_box, stack, playlists }
    }

    /// Replaces the playlist list with `playlists`.
    pub fn set_playlists(&self, playlists: Vec<Playlist>) {
        while let Some(child) = self.list_box.first_child() {
            self.list_box.remove(&child);
        }

        if playlists.is_empty() {
            *self.playlists.borrow_mut() = playlists;
            self.stack.set_visible_child_name("empty");
            return;
        }

        for playlist in &playlists {
            self.list_box.append(&build_playlist_row(playlist));
        }

        *self.playlists.borrow_mut() = playlists;
        self.stack.set_visible_child_name("playlists");
    }
}

fn build_playlist_row(playlist: &Playlist) -> gtk4::ListBoxRow {
    let art = gtk4::Picture::builder()
        .css_classes(vec!["playlist-art"])
        .width_request(40)
        .height_request(40)
        .content_fit(gtk4::ContentFit::Cover)
        .can_shrink(true)
        .build();
    art.set_overflow(gtk4::Overflow::Hidden);
    cover_art::set_picture_from_url(&art, playlist.image.clone());

    let title = gtk4::Label::builder()
        .css_classes(vec!["row-title"])
        .label(&playlist.name)
        .halign(gtk4::Align::Start)
        .ellipsize(gtk4::pango::EllipsizeMode::End)
        .build();
    let subtitle = gtk4::Label::builder()
        .css_classes(vec!["row-meta"])
        .label(format!("{} tracks", playlist.track_count))
        .halign(gtk4::Align::Start)
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
