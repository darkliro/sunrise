//! The reset and template screens: `RealmsResetWorldScreen` (`ResetWorld`) offering the
//! 3x2 grid to generate, upload or reset from a template, and
//! `RealmsSelectWorldTemplateScreen` (`TemplateList`) paginating through the templates.
//!
//! `realms.rs` keeps the `SubScreen` variants and dispatch calls; the arms and the methods
//! that are the reset/template screens' are here.

use super::client::{
    RealmServer, RealmSetting, RealmSlot, RealmWorldOptions, RealmsClient, RealmsError,
    WorldTemplate,
};
use super::preset_world;
use super::{
    ConfigureTab, GREY, Outcome, REALMS_CONTENT_CREATION, RED, ROW_WIDTH, RunningTask, Screen,
    SubScreen, TEMPLATE_ROW, centered, client, icon_sprite, loading_dots, realms_logo, spawn, take,
    task_stack_top, template_image, upload, wrap_lines,
};
use crate::hud::{HudAtlas, HudBatch};
use crate::text::Text;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

/// `RealmsResetWorldScreen`'s entry points (`forResetSlot`, `forEmptySlot`, `forNewRealm`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResetKind {
    /// `forResetSlot`: the world is replaced after a warning; the active slot's.
    Reset,
    /// `forEmptySlot`: the slot has no world yet.
    Empty,
    /// `forNewRealm`: the Realm itself does not exist yet; the reset creates it first
    /// (`RealmCreationTask`).
    NewRealm,
}

impl ResetKind {
    /// The screen's title (`RealmsResetWorldScreen`'s constants).
    fn title(self) -> &'static str {
        match self {
            Self::Reset => "mco.reset.world.title",
            Self::Empty => "mco.configure.world.switch.slot",
            Self::NewRealm => "mco.selectServer.create",
        }
    }
    /// The subtitle, and whether it is the red warning.
    fn subtitle(self) -> (&'static str, bool) {
        match self {
            Self::Reset => ("mco.reset.world.warning", true),
            Self::Empty => ("mco.configure.world.switch.slot.subtitle", false),
            Self::NewRealm => ("mco.selectServer.create.subtitle", false),
        }
    }
    /// `RESET_WORLD_RESET_TASK_TITLE` and `CREATE_WORLD_RESET_TASK_TITLE`.
    fn task_title(self) -> &'static str {
        match self {
            Self::Reset => "mco.reset.world.resetting.screen.title",
            Self::Empty | Self::NewRealm => "mco.create.world.reset.title",
        }
    }
}

/// The four template groups of `RealmsResetWorldScreen`'s grid, in its order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TemplateGroup {
    Normal,
    Adventure,
    Experience,
    Inspiration,
}

/// `RealmsCreateWorldFlow.createWorld`'s target: the reset screen's Generate New World
/// tile asks the title for the Create New World screen, whose settings upload into
/// `slot`.
#[derive(Clone, Debug)]
pub struct CreateWorldRequest {
    pub server: RealmServer,
    pub slot: i32,
    pub kind: ResetKind,
    /// `RealmCreationTask`'s name and description, for `ResetKind::NewRealm`.
    pub creation: Option<(String, String)>,
}

pub use preset_world::CreatedWorld;

impl TemplateGroup {
    const ALL: [Self; 4] = [
        Self::Normal,
        Self::Adventure,
        Self::Experience,
        Self::Inspiration,
    ];

    fn index(self) -> usize {
        Self::ALL
            .iter()
            .position(|g| *g == self)
            .expect("a group is in ALL")
    }
    /// `RealmsServer.WorldType`'s name: what `fetchWorldTemplates` puts in the path.
    fn world_type(self) -> &'static str {
        match self {
            Self::Normal => "NORMAL",
            Self::Adventure => "ADVENTUREMAP",
            Self::Experience => "EXPERIENCE",
            Self::Inspiration => "INSPIRATION",
        }
    }
    fn title(self) -> &'static str {
        match self {
            Self::Normal => "mco.reset.world.template",
            Self::Adventure => "mco.reset.world.adventure",
            Self::Experience => "mco.reset.world.experience",
            Self::Inspiration => "mco.reset.world.inspiration",
        }
    }
}

/// `RealmsCreateWorldFlow.createWorld`'s worker: `createTemporaryWorldFolder`, then
/// `RealmCreationTask` → `SwitchSlotTask` → `packAndUpload`, as an upload's worker does.
fn upload_created_world(
    client: &RealmsClient,
    server: &RealmServer,
    slot: i32,
    creation: Option<&(String, String)>,
    world: &CreatedWorld,
    progress: &upload::UploadProgress,
) -> Result<(), upload::UploadError> {
    let dir = create_temporary_world(world).map_err(upload::UploadError::Failed)?;
    let result = (|| {
        if let Some((name, description)) = creation {
            client
                .initialize_realm(server.id, name, description)
                .map_err(|e| upload::UploadError::Failed(e.text()))?;
        }
        if slot != server.active_slot {
            client
                .switch_slot(server.id, slot)
                .map_err(|e| upload::UploadError::Failed(e.text()))?;
        }
        // `RealmsWorldOptions.createFromSettings`: the world's own settings, the game's
        // version as the world version.
        let options = RealmWorldOptions {
            spawn_protection: 0,
            force_game_mode: false,
            difficulty: i32::from(world.level.difficulty_id()),
            game_mode: if world.level.creative { 1 } else { 0 },
            slot_name: world.level.name.clone(),
            version: crate::save::VERSION_NAME.into(),
            compatibility: "UNVERIFIABLE".into(),
            template_id: -1,
            template_image: None,
            empty: false,
        };
        let realm_slot = RealmSlot {
            slot_id: slot,
            options,
            settings: vec![RealmSetting {
                name: "hardcore".into(),
                value: world.level.hardcore.to_string(),
            }],
        };
        upload::upload_world(client, server.id, &realm_slot, &dir, progress)
    })();
    let _ = std::fs::remove_dir_all(&dir);
    result
}

/// `RealmsCreateWorldFlow.createTemporaryWorldFolder`: level.dat as
/// `WorldData.createTag(null)` writes it (no `singleplayer_uuid`), the world generation
/// and game rules, and the Create World screen's data packs moved into the world. No
/// chunks: the Realm generates them when the world first opens.
fn create_temporary_world(world: &CreatedWorld) -> std::result::Result<PathBuf, String> {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let dir = std::env::temp_dir().join(format!(
        "sunrise-realms-world-upload-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = std::fs::remove_dir_all(&dir);
    let result = (|| {
        let mut level = world.level.clone();
        level.data_packs = Some(world.data_packs.clone());
        level.write_upload(&dir).map_err(|e| e.to_string())?;
        crate::save::write_world_gen_settings_with(
            &dir,
            level.seed,
            world.world_gen,
            world.flat_settings.clone(),
            world.single_biome.as_deref(),
            world.dimensions.clone(),
        )
        .map_err(|e| e.to_string())?;
        if let Some(pack_dir) = &world.pack_dir {
            // `Files.move(tempDataPackDir, worldFolder.resolve("datapacks"))`; the
            // Create World screen's temporary folder is gone afterwards.
            crate::datapack::install(pack_dir, &dir, &world.data_packs.0)?;
            let _ = std::fs::remove_dir_all(pack_dir);
        }
        Ok(())
    })();
    match result {
        Ok(()) => Ok(dir),
        Err(e) => {
            let _ = std::fs::remove_dir_all(&dir);
            Err(e)
        }
    }
}

/// `RealmsSelectWorldTemplateScreen.fetchTemplates`: one page at a time; the list shows it
/// as it arrives and the screen chains the next page until the last or an empty one.
fn fetch_template_page(
    client: &RealmsClient,
    world_type: &str,
    page: i32,
    page_size: i32,
) -> Result<(Vec<WorldTemplate>, i32, bool), RealmsError> {
    let answer = client.fetch_world_templates(page, page_size, world_type)?;
    let empty = answer.templates.is_empty();
    Ok((
        answer.templates,
        page + 1,
        empty || last_template_page(answer.page, answer.size, answer.total),
    ))
}

/// `WorldTemplatePaginatedList.isLastPage`.
fn last_template_page(page: i32, size: i32, total: i32) -> bool {
    page > 0 && size > 0 && total > 0 && page * size >= total
}

/// `TextRenderingUtils.decompose(I18n.get("mco.template.select.none", "%link"), link)`:
/// the translated message's lines, the one holding the `%s` argument split into
/// `before`/`link`/`after`; every other line is whole (`link` is None).
fn template_none_lines(text: &str, link: &str) -> Vec<(String, Option<String>, String)> {
    text.replacen("%s", "%link", 1)
        .split('\n')
        .map(|line| match line.split_once("%link") {
            Some((before, after)) => (before.to_owned(), Some(link.to_owned()), after.to_owned()),
            None => (line.to_owned(), None, String::new()),
        })
        .collect()
}

/// `RealmsResetWorldScreen.init`'s grid: three 60-wide tiles with 16 padding a side, and a
/// 16-tall spacer between the two rows (`FrameButton` is 60 wide and 69 tall: the button
/// plus its 9-tall label). The contents frame starts at `min(63, h - 187)`.
fn reset_content_y(h: f32) -> f32 {
    63.0_f32.min(h - 187.0)
}

fn reset_tile(cx: f32, top: f32, i: usize) -> (f32, f32) {
    let column = (i % 3) as f32;
    let row = (i / 3) as f32;
    (cx - 122.0 + column * 92.0, top + row * 85.0)
}

/// The row `y` falls on in a list that starts at `top` and has scrolled `offset` pixels.
fn row_at(
    x: f32,
    y: f32,
    w: f32,
    top: f32,
    bottom: f32,
    height: f32,
    offset: f32,
) -> Option<usize> {
    let rx = ((w - ROW_WIDTH) / 2.0).floor();
    if y < top || y >= bottom || x < rx || x > rx + ROW_WIDTH {
        return None;
    }
    let row = (y - top - 2.0 + offset) / height;
    (row >= 0.0).then(|| row.floor() as usize)
}

impl Screen {
    pub(super) fn poll_reset_world(&mut self) -> Option<Outcome> {
        let mut back_to_worlds: Option<RealmServer> = None;
        let mut refresh = false;
        let mut created = false;
        let mut task_error: Option<String> = None;

        match &mut self.subscreen {
            SubScreen::ResetWorld {
                server,
                kind,
                templates,
                job,
                reset,
                ..
            } => {
                if let Some(result) = job.as_ref().and_then(take) {
                    *job = None;
                    *templates = Some(result);
                }
                if let Some(result) = reset.as_ref().map(|task| &task.job).and_then(take) {
                    *reset = None;
                    match result {
                        // `resetWorldRunnable`: the configuration screen again, the list
                        // reloaded (`RealmsMainScreen.refreshServerList`); a new Realm goes
                        // back to the list it was created from.
                        Ok(()) => match kind {
                            ResetKind::NewRealm => {
                                created = true;
                            }
                            _ => {
                                refresh = true;
                                back_to_worlds = Some(server.clone());
                            }
                        },
                        Err(e) => task_error = Some(e.text()),
                    }
                }
            }
            SubScreen::TemplateList {
                server,
                templates,
                job,
                group,
                select,
                error,
                ..
            } => {
                if let Some(result) = job.as_ref().and_then(take) {
                    *job = None;
                    match result {
                        // `fetchTemplatesAsync`: each page joins the list as it arrives;
                        // the next one is asked for while the answer is not the last page.
                        Ok((page, next_page, last)) => {
                            templates.extend(page);
                            if !last && let Some(client) = client() {
                                let world_type = group
                                    .map_or("MINIGAME", TemplateGroup::world_type)
                                    .to_owned();
                                *job = Some(spawn(move || {
                                    fetch_template_page(&client, &world_type, next_page, 10)
                                }));
                            }
                        }
                        // `fetchTemplatesAsync`: a page failure keeps the list and shows
                        // `mco.template.select.failure` when nothing arrived.
                        Err(e) => *error = Some(e.text()),
                    }
                }
                if let Some(result) = select.as_ref().map(|task| &task.job).and_then(take) {
                    *select = None;
                    match result {
                        // `SwitchMinigameTask`: the configuration screen again.
                        Ok(()) => back_to_worlds = Some(server.clone()),
                        Err(e) => task_error = Some(e.text()),
                    }
                }
            }
            _ => return None,
        }

        if let Some(detail) = task_error {
            self.show_generic_error(detail);
        }
        if created {
            self.subscreen = SubScreen::Main;
            self.selected = None;
            self.refresh();
        }
        if let Some(server) = back_to_worlds {
            self.back_to_configure_worlds(server);
        }
        if refresh {
            self.refresh();
        }
        Some(Outcome::None)
    }

    pub(super) fn escape_reset_world(&mut self) -> Option<bool> {
        enum Escape {
            Reset(RealmServer, i32, ResetKind, Option<(String, String)>),
            Configure(RealmServer),
            CancelTask(RealmServer),
        }
        let action = match &self.subscreen {
            SubScreen::ResetWorld { server, reset, .. } => {
                if let Some(task) = reset {
                    task.abort.store(true, Ordering::Relaxed);
                    Escape::CancelTask(server.clone())
                } else {
                    Escape::Configure(server.clone())
                }
            }
            SubScreen::TemplateList {
                server,
                slot,
                kind,
                creation,
                group,
                select,
                ..
            } => {
                if let Some(task) = select {
                    // The minigame switch runs in the task screen: Escape cancels it.
                    task.abort.store(true, Ordering::Relaxed);
                    Escape::CancelTask(server.clone())
                } else if group.is_none() {
                    // `RealmsWorldsTab.switchToMinigame`'s callback: the list belongs to
                    // the configuration screen, not to a reset screen.
                    Escape::Configure(server.clone())
                } else {
                    Escape::Reset(server.clone(), *slot, *kind, creation.clone())
                }
            }
            _ => return None,
        };
        match action {
            Escape::Reset(server, slot, kind, creation) => {
                self.subscreen = SubScreen::ResetWorld {
                    server,
                    slot,
                    kind,
                    creation,
                    templates: None,
                    job: None,
                    reset: None,
                    error: None,
                };
            }
            Escape::Configure(server) | Escape::CancelTask(server) => {
                self.back_to_configure_worlds(server);
            }
        }
        Some(false)
    }

    pub(super) fn click_reset_world(
        &mut self,
        hit: Option<u8>,
        _x: f32,
        _y: f32,
        (_w, _h): (f32, f32),
    ) -> Option<Outcome> {
        match hit {
            Some(tile @ 70..=75) => {
                if let SubScreen::ResetWorld {
                    server,
                    slot,
                    kind,
                    creation,
                    reset: None,
                    ..
                } = &self.subscreen
                {
                    let (server, slot, kind) = (server.clone(), *slot, *kind);
                    let creation = creation.clone();
                    if tile == 70 {
                        // `RealmsCreateWorldFlow.createWorld`: the title opens its Create
                        // New World screen; the settings come back for the upload.
                        return Some(Outcome::CreateWorld(CreateWorldRequest {
                            server,
                            slot,
                            kind,
                            creation,
                        }));
                    }
                    if tile == 71 {
                        self.open_upload_select(server, slot, kind, creation);
                    } else if let Some(group) = TemplateGroup::ALL.get(usize::from(tile - 72)) {
                        self.open_template_list(server, slot, kind, creation, Some(*group));
                    }
                }
                Some(Outcome::None)
            }
            Some(76) => {
                if let SubScreen::ResetWorld { server, .. } = &self.subscreen {
                    let server = server.clone();
                    self.back_to_configure_worlds(server);
                }
                Some(Outcome::None)
            }
            Some(88) => {
                let server = match &self.subscreen {
                    SubScreen::ResetWorld { server, reset, .. } => {
                        if let Some(task) = reset {
                            task.abort.store(true, Ordering::Relaxed);
                        }
                        Some(server.clone())
                    }
                    SubScreen::TemplateList { server, select, .. } => {
                        if let Some(task) = select {
                            task.abort.store(true, Ordering::Relaxed);
                        }
                        Some(server.clone())
                    }
                    _ => None,
                };
                if let Some(server) = server {
                    self.back_to_configure_worlds(server);
                }
                Some(Outcome::None)
            }
            Some(77) => {
                self.select_template();
                Some(Outcome::None)
            }
            Some(78) => {
                if let SubScreen::TemplateList {
                    server,
                    slot,
                    kind,
                    creation,
                    group,
                    ..
                } = &self.subscreen
                {
                    let (server, slot, kind, group) = (server.clone(), *slot, *kind, *group);
                    let creation = creation.clone();
                    // `RealmsWorldsTab.switchToMinigame`'s callback sends a cancelled
                    // minigame list back to the configuration screen; the reset screen's
                    // own list returns to the reset screen (`realmCreationTask` kept).
                    if group.is_none() {
                        self.back_to_configure_worlds(server);
                    } else {
                        self.subscreen = SubScreen::ResetWorld {
                            server,
                            slot,
                            kind,
                            creation,
                            templates: None,
                            job: None,
                            reset: None,
                            error: None,
                        };
                    }
                }
                Some(Outcome::None)
            }
            Some(which @ (79 | 92)) => {
                if let SubScreen::TemplateList {
                    templates,
                    selected,
                    ..
                } = &self.subscreen
                    && let Some(template) = selected.and_then(|i| templates.get(i))
                {
                    let link = if which == 79 {
                        template.trailer.clone()
                    } else {
                        template.link.clone()
                    };
                    if let Some(link) = link {
                        self.open_link(link);
                    }
                }
                Some(Outcome::None)
            }
            _ => None,
        }
    }

    pub(super) fn click_reset_list(
        &mut self,
        x: f32,
        y: f32,
        (w, h): (f32, f32),
    ) -> Option<Outcome> {
        // `RealmsSelectWorldTemplateScreen.WorldTemplateList.mouseClicked`: the empty
        // list's link asks before opening the content-creation page.
        if matches!(self.subscreen, SubScreen::TemplateList { .. }) {
            let [lx, ly, lw, lh] = self.template_link_rect.get();
            if lw > 0.0 && x >= lx && x < lx + lw && y >= ly && y < ly + lh {
                self.open_link(REALMS_CONTENT_CREATION.to_owned());
                return Some(Outcome::None);
            }
        }
        // `RealmsResetWorldScreen`'s tiles are drawn, not widgets: the click is a rectangle.
        if let SubScreen::ResetWorld { reset, .. } = &self.subscreen {
            if reset.is_some() {
                return Some(Outcome::None);
            }
            let cx = (w / 2.0).floor();
            let top = reset_content_y(h);
            // `FrameButton` is 60 wide and 69 tall (its label is part of the control).
            let tile = (0..6).find(|i| {
                let (tx, ty) = reset_tile(cx, top, *i);
                x >= tx && x < tx + 60.0 && y >= ty && y < ty + 69.0
            });
            if let Some(tile) = tile {
                return Some(self.click(Some(70 + tile as u8), x, y, (w, h)));
            }
            return Some(Outcome::None);
        }
        // `Entry.mouseClicked` checks its two link buttons before the list; the question
        // (`ConfirmLinkScreen.confirmLink`) is asked by the popup.
        let link = match &self.subscreen {
            SubScreen::TemplateList {
                templates, icons, ..
            } => icons.borrow().iter().find_map(|(row, rect, trailer)| {
                let [ix, iy, iw, ih] = *rect;
                if x < ix || x >= ix + iw || y < iy || y >= iy + ih {
                    return None;
                }
                let template = templates.get(*row)?;
                if *trailer {
                    template.trailer.clone()
                } else {
                    template.link.clone()
                }
            }),
            _ => None,
        };
        if let Some(link) = link {
            self.open_link(link);
            return Some(Outcome::None);
        }
        let picked = match &self.subscreen {
            SubScreen::TemplateList {
                group,
                templates,
                scroll,
                select,
                ..
            } if select.is_none() => {
                let top = if group.is_none() { 59.0 } else { 33.0 };
                row_at(x, y, w, top, h - 33.0, TEMPLATE_ROW, scroll.amount)
                    .filter(|row| *row < templates.len())
                    .map(|row| (row, true))
            }
            _ => None,
        };
        if let Some((row, template)) = picked {
            let now = SystemTime::now();
            let double = matches!(self.last_click, Some((last, at)) if last == row
                && now.duration_since(at).is_ok_and(|d| d.as_millis() < 250));
            self.last_click = Some((row, now));
            match &mut self.subscreen {
                SubScreen::TemplateList { selected, .. } => *selected = Some(row),
                _ => {}
            }
            if double && template {
                self.select_template();
            }
            return Some(Outcome::None);
        }
        None
    }

    pub(super) fn wheel_reset_world(&mut self, delta: f32, cursor: (f32, f32), h: f32) -> bool {
        let (top, bottom, content, height) = match &self.subscreen {
            SubScreen::TemplateList {
                group, templates, ..
            } => (
                if group.is_none() { 59.0 } else { 33.0 },
                h - 33.0,
                templates.len() as f32 * TEMPLATE_ROW,
                TEMPLATE_ROW,
            ),
            _ => return false,
        };
        if cursor.1 < top || cursor.1 >= bottom {
            return false;
        }
        match &mut self.subscreen {
            SubScreen::TemplateList { scroll, .. } => {
                scroll.wheel(delta, height / 2.0, top, bottom, content);
                true
            }
            _ => false,
        }
    }

    pub(super) fn reset_world_buttons(
        &self,
        w: f32,
        h: f32,
        buttons: &mut Vec<(f32, f32, f32, String, bool, u8, Option<(&'static str, f32)>)>,
    ) {
        let cx = (w / 2.0).floor();
        let t = crate::lang::get;
        match &self.subscreen {
            SubScreen::ResetWorld { reset, .. } => {
                if reset.is_some() {
                    // `RealmsLongRunningMcoTaskScreen`: the tasks' screen only offers Cancel.
                    let top = task_stack_top((w, h));
                    buttons.push((
                        cx - 75.0,
                        top + 102.0,
                        150.0,
                        t("gui.cancel"),
                        true,
                        88,
                        None,
                    ));
                } else {
                    buttons.push((cx - 75.0, h - 27.0, 150.0, t("gui.back"), true, 76, None));
                }
            }
            SubScreen::TemplateList {
                templates,
                selected,
                select,
                ..
            } => {
                if select.is_some() {
                    // `RealmsLongRunningMcoTaskScreen` replaced the list while the minigame
                    // switch runs: only its Cancel.
                    let top = task_stack_top((w, h));
                    buttons.push((
                        cx - 75.0,
                        top + 102.0,
                        150.0,
                        t("gui.cancel"),
                        true,
                        88,
                        None,
                    ));
                    return;
                }
                // `RealmsSelectWorldTemplateScreen`'s footer: trailer, select, cancel and the
                // publisher page, four 100-wide buttons 8 apart (424 total, or 212 either
                // side of the centre), at h - 27. Trailer and publisher are `visible =
                // false` until a template with those links is selected — their slots stay
                // reserved, they just do not draw or focus.
                let chosen = selected.and_then(|i| templates.get(i));
                let x = |i: usize| cx - 212.0 + i as f32 * 108.0;
                if chosen.is_some_and(|template| template.trailer.is_some()) {
                    buttons.push((
                        x(0),
                        h - 27.0,
                        100.0,
                        t("mco.template.button.trailer"),
                        true,
                        79,
                        None,
                    ));
                }
                buttons.push((
                    x(1),
                    h - 27.0,
                    100.0,
                    t("mco.template.button.select"),
                    chosen.is_some() && select.is_none(),
                    77,
                    None,
                ));
                buttons.push((x(2), h - 27.0, 100.0, t("gui.cancel"), true, 78, None));
                if chosen.is_some_and(|template| template.link.is_some()) {
                    buttons.push((
                        x(3),
                        h - 27.0,
                        100.0,
                        t("mco.template.button.publisher"),
                        true,
                        92,
                        None,
                    ));
                }
            }
            _ => {}
        }
    }

    pub(super) fn draw_reset_world(
        &self,
        batch: &mut HudBatch,
        atlas: &HudAtlas,
        (w, h): (f32, f32),
        cursor: (f32, f32),
    ) {
        let SubScreen::ResetWorld {
            kind, reset, error, ..
        } = &self.subscreen
        else {
            return;
        };
        let cx = (w / 2.0).floor();
        let t = crate::lang::get;
        if let Some(task) = reset {
            // `RealmsLongRunningMcoTaskScreen`: the tasks' screen replaced this one —
            // `realmsLogo`, then the `LoadingDotsWidget` (its message at widget +2,
            // the grey dots at +17), then the Cancel 102 into the 122-tall stack.
            let top = task_stack_top((w, h));
            realms_logo(batch, atlas, cx, top);
            let title = task.title.lock().expect("a task title").clone();
            centered(batch, atlas, &title, cx, top + 46.0, u32::MAX);
            centered(batch, atlas, loading_dots(), cx, top + 61.0, 0xFF80_8080);
        } else {
            // `init`: the header's two 9-tall lines with 3 of padding, centred in
            // the 33-tall frame — title 4, subtitle 19.
            let (title, subtitle) = (kind.title(), kind.subtitle());
            centered(batch, atlas, &t(title), cx, 4.0, u32::MAX);
            centered(
                batch,
                atlas,
                &t(subtitle.0),
                cx,
                19.0,
                if subtitle.1 { RED } else { GREY },
            );
            if let Some(e) = error {
                // The reset screen's own error line, under the tiles.
                for (i, line) in wrap_lines(e, atlas, w - 40.0).iter().enumerate() {
                    centered(batch, atlas, line, cx, h - 56.0 + i as f32 * 10.0, RED);
                }
            }
            // `RealmsResetWorldScreen.init`'s tiles: Generate New World, Upload, the
            // four groups. Generate New World opens Create New World
            // (`RealmsCreateWorldFlow.createWorld`) and uploads what it made.
            let tiles = [
                ("mco.reset.world.generate", "gui/realms/new_world"),
                ("mco.upload.select.world.title", "gui/realms/upload"),
                ("mco.reset.world.template", "gui/realms/survival_spawn"),
                ("mco.reset.world.adventure", "gui/realms/adventure"),
                ("mco.reset.world.experience", "gui/realms/experience"),
                ("mco.reset.world.inspiration", "gui/realms/inspiration"),
            ];
            let content_y = reset_content_y(h);
            for (i, (label, image)) in tiles.iter().enumerate() {
                let (x, y) = reset_tile(cx, content_y, i);
                // The control is 60x69: the frame plus its label.
                let hover = crate::ui::hovered(cursor, x, y, 60.0, 69.0);
                let frame = if hover { 0xFF90_9090 } else { u32::MAX };
                batch.fill(atlas, x + 2.0, y + 2.0, 56.0, 56.0, 0xFF20_2020);
                // `FrameButton`: the picture inside `widget/slot_frame`.
                crate::ui::blit_sprite(batch, atlas, image, x + 2.0, y + 2.0, 56.0, 56.0, frame);
                crate::ui::blit_sprite(
                    batch,
                    atlas,
                    "gui/sprites/widget/slot_frame",
                    x,
                    y,
                    60.0,
                    60.0,
                    frame,
                );
                let color = if hover { GREY } else { u32::MAX };
                let text = t(label);
                let width = HudBatch::text_width(atlas, &text);
                // `FrameButton.extractContents`: `centeredText(..., x + 28, y - 14)`.
                batch.text(
                    atlas,
                    &text,
                    x + 28.0 - (width / 2.0).floor(),
                    y - 14.0,
                    color,
                    true,
                );
            }
        }
    }

    pub(super) fn draw_template_list(
        &self,
        batch: &mut HudBatch,
        atlas: &HudAtlas,
        (w, h): (f32, f32),
        cursor: (f32, f32),
        list_focused: bool,
    ) {
        let SubScreen::TemplateList {
            group,
            templates,
            selected,
            scroll,
            job,
            select,
            icons,
            error,
            ..
        } = &self.subscreen
        else {
            return;
        };
        let cx = (w / 2.0).floor();
        let t = crate::lang::get;
        if let Some(task) = select {
            // `RealmsLongRunningMcoTaskScreen` replaced the list while the switch
            // runs: its logo, the task's title and grey dots, and Cancel.
            let top = task_stack_top((w, h));
            realms_logo(batch, atlas, cx, top);
            let title = task.title.lock().expect("a task title").clone();
            centered(batch, atlas, &title, cx, top + 46.0, u32::MAX);
            centered(batch, atlas, loading_dots(), cx, top + 61.0, 0xFF80_8080);
        } else {
            let title = group.map_or("mco.template.title.minigame", TemplateGroup::title);
            centered(batch, atlas, &t(title), cx, 12.0, u32::MAX);
            // `init`: header height 33 + 13 per subtitle line (the minigame list
            // has two); the list starts at the header's height.
            let top = if group.is_none() { 59.0 } else { 33.0 };
            if group.is_none() {
                // `mco.minigame.world.info.line1`/`line2`: the switch's two
                // warnings, grey (-4539718), 13 apart.
                for (i, key) in [
                    "mco.minigame.world.info.line1",
                    "mco.minigame.world.info.line2",
                ]
                .iter()
                .enumerate()
                {
                    centered(
                        batch,
                        atlas,
                        &t(key),
                        cx,
                        25.0 + i as f32 * 13.0,
                        0xFFBA_BABA,
                    );
                }
            }
            let bottom = h - 33.0;
            crate::ui::list_background(batch, atlas, top, bottom, false);
            // `ObjectSelectionList` separators at the list's edges.
            crate::ui::tiled_sized(
                batch,
                atlas,
                "gui/header_separator",
                0.0,
                top - 2.0,
                w,
                2.0,
                [32.0, 2.0],
                u32::MAX,
            );
            crate::ui::tiled_sized(
                batch,
                atlas,
                "gui/footer_separator",
                0.0,
                bottom,
                w,
                2.0,
                [32.0, 2.0],
                u32::MAX,
            );
            let rx = ((w - ROW_WIDTH) / 2.0).floor();
            let content = templates.len() as f32 * TEMPLATE_ROW;
            let mut icons = icons.borrow_mut();
            icons.clear();
            let first = batch.vertices.len();
            for (i, template) in templates.iter().enumerate() {
                let y = top + 2.0 + i as f32 * TEMPLATE_ROW - scroll.amount;
                if y + TEMPLATE_ROW <= top || y >= bottom {
                    continue;
                }
                if *selected == Some(i) {
                    // `AbstractSelectionList.extractSelection`: the whole entry,
                    // white while the list is focused.
                    batch.fill(
                        atlas,
                        rx,
                        y,
                        ROW_WIDTH,
                        TEMPLATE_ROW,
                        if list_focused {
                            0xFFFF_FFFF
                        } else {
                            0xFF80_8080
                        },
                    );
                    batch.fill(
                        atlas,
                        rx + 1.0,
                        y + 1.0,
                        ROW_WIDTH - 2.0,
                        TEMPLATE_ROW - 2.0,
                        0xFF00_0000,
                    );
                }
                // `RealmsTextureManager.worldTemplate`: the service's base64 PNG,
                // decoded to a 64x64 icon slot and drawn 38x38 inside the frame.
                let pixels = template.image.as_ref().and_then(|image| {
                    let key = format!("{}:{}", template.id, image.len());
                    let mut cache = self.template_images.borrow_mut();
                    cache
                        .entry(key)
                        .or_insert_with(|| template_image(image))
                        .clone()
                });
                let picture = pixels
                    .as_deref()
                    .and_then(|rgba| icon_sprite(batch, atlas, i, rgba));
                match picture {
                    Some(sprite) => {
                        batch.sprite_scaled(
                            atlas,
                            &sprite,
                            rx + 3.0,
                            y + 4.0,
                            38.0,
                            38.0,
                            u32::MAX,
                        );
                    }
                    None => {
                        batch.fill(atlas, rx + 3.0, y + 4.0, 38.0, 38.0, 0xFF20_2020);
                    }
                }
                crate::ui::blit_sprite(
                    batch,
                    atlas,
                    "gui/sprites/widget/slot_frame",
                    rx + 2.0,
                    y + 3.0,
                    40.0,
                    40.0,
                    u32::MAX,
                );
                batch.text(atlas, &template.name, rx + 67.0, y + 7.0, u32::MAX, true);
                let version = HudBatch::text_width(atlas, &template.version);
                batch.text(
                    atlas,
                    &template.version,
                    rx + 298.0 - version - 5.0,
                    y + 7.0,
                    GREY,
                    true,
                );
                // `Entry`: the website and trailer buttons, left of the version.
                for (trailer, offset) in [(false, 10.0), (true, 15.0 + 15.0)] {
                    let present = if trailer {
                        template.trailer.is_some()
                    } else {
                        template.link.is_some()
                    };
                    if !present {
                        continue;
                    }
                    // `Entry.extractContent`: `setPosition(..., getContentY())`.
                    let (x, icon_y) = (rx + 298.0 - version - 15.0 - offset, y + 2.0);
                    let hovered = crate::ui::hovered(cursor, x, icon_y, 15.0, 15.0);
                    let sprite = match (trailer, hovered) {
                        (false, false) => "gui/sprites/icon/link",
                        (false, true) => "gui/sprites/icon/link_highlighted",
                        (true, false) => "gui/sprites/icon/video_link",
                        (true, true) => "gui/sprites/icon/video_link_highlighted",
                    };
                    crate::ui::blit_sprite(batch, atlas, sprite, x, icon_y, 15.0, 15.0, u32::MAX);
                    icons.push((i, [x, icon_y, 15.0, 15.0], trailer));
                }
                batch.text(atlas, &template.author, rx + 67.0, y + 21.0, GREY, true);
                if !template.recommended_players.is_empty() {
                    batch.text(
                        atlas,
                        &template.recommended_players,
                        rx + 67.0,
                        y + 35.0,
                        0xFF80_8080,
                        true,
                    );
                }
            }
            batch.clip_since(first, 0.0, top, w, bottom);
            crate::ui::scrollbar(
                batch,
                atlas,
                rx + ROW_WIDTH + 8.0,
                top,
                bottom,
                content,
                scroll.amount,
            );
            if templates.is_empty() {
                if error.is_some() {
                    // `RealmsSelectWorldTemplateScreen`: a failed fetch says so with
                    // `mco.template.select.failure`, not the error's text.
                    centered(
                        batch,
                        atlas,
                        &t("mco.template.select.failure"),
                        cx,
                        top + 10.0,
                        u32::MAX,
                    );
                } else if job.is_some() {
                    centered(batch, atlas, loading_dots(), cx, top + 10.0, GREY);
                } else {
                    // `RealmsSelectWorldTemplateScreen` asks
                    // `I18n.get("mco.template.select.none", "%link")` and decomposes
                    // the result (`TextRenderingUtils.decompose`): only the line with
                    // the `%s` argument carries the link (the raw translation has
                    // `%s`, not `%link`), in -13408581 (0xFF3366BB), the rest white,
                    // each line centred; hovering the link shows its URL.
                    let lines = template_none_lines(
                        &t("mco.template.select.none"),
                        &t("mco.template.select.none.linkTitle"),
                    );
                    let mut link_rect = [0.0; 4];
                    for (i, (before, link, after)) in lines.iter().enumerate() {
                        let y = 92.0 + i as f32 * 13.0;
                        let width = HudBatch::text_width(atlas, before)
                            + link
                                .as_deref()
                                .map_or(0.0, |link| HudBatch::text_width(atlas, link))
                            + HudBatch::text_width(atlas, after);
                        let mut x = cx - (width / 2.0).floor();
                        batch.text(atlas, before, x, y, u32::MAX, true);
                        x += HudBatch::text_width(atlas, before);
                        if let Some(link) = link {
                            batch.text(atlas, link, x, y, 0xFF33_66BB, true);
                            let link_w = HudBatch::text_width(atlas, link);
                            if crate::ui::hovered(cursor, x, y - 3.0, link_w, 11.0) {
                                crate::ui::tooltip(
                                    batch,
                                    atlas,
                                    &[Text::literal(REALMS_CONTENT_CREATION)],
                                    cursor,
                                );
                                link_rect = [x, y - 3.0, link_w, 11.0];
                            }
                            x += link_w;
                        }
                        batch.text(atlas, after, x, y, u32::MAX, true);
                    }
                    self.template_link_rect.set(link_rect);
                }
            }
        }
    }

    /// `RealmsWorldsTab.resetButtonPressed`: a minigame Realm picks another template, any other
    /// Realm replaces the active slot's world.
    pub(super) fn reset_button(&mut self) {
        let SubScreen::Configure {
            server,
            tab: ConfigureTab::Worlds,
            detail: Some(Ok(detail)),
            ..
        } = &self.subscreen
        else {
            return;
        };
        let server = server.clone();
        let slot = server.active_slot;
        if server.world_type == "MINIGAME" {
            self.open_template_list(server, slot, ResetKind::Reset, None, None);
            return;
        }
        let empty = detail
            .slots
            .get(&detail.active_slot)
            .is_none_or(|slot| slot.options.empty);
        let kind = if empty {
            ResetKind::Empty
        } else {
            ResetKind::Reset
        };
        self.open_reset_world(server, slot, kind);
    }

    /// `new RealmsResetWorldScreen`: `init` asks the service for page 1 of each group.
    pub(super) fn open_reset_world(&mut self, server: RealmServer, slot: i32, kind: ResetKind) {
        let job = client().map(|client| {
            spawn(move || {
                [
                    client.fetch_world_templates(1, 10, "NORMAL"),
                    client.fetch_world_templates(1, 10, "ADVENTUREMAP"),
                    client.fetch_world_templates(1, 10, "EXPERIENCE"),
                    client.fetch_world_templates(1, 10, "INSPIRATION"),
                ]
            })
        });
        self.subscreen = SubScreen::ResetWorld {
            server,
            slot,
            kind,
            creation: None,
            templates: None,
            job,
            reset: None,
            error: None,
        };
    }

    /// `new RealmsSelectWorldTemplateScreen`: its first page (the reset screen's when it has
    /// one) and the rest of the pages, then the list.
    pub(super) fn open_template_list(
        &mut self,
        server: RealmServer,
        slot: i32,
        kind: ResetKind,
        creation: Option<(String, String)>,
        group: Option<TemplateGroup>,
    ) {
        let prefetched = match (&self.subscreen, group) {
            (
                SubScreen::ResetWorld {
                    templates: Some(lists),
                    ..
                },
                Some(wanted),
            ) => lists[wanted.index()].clone().ok(),
            _ => None,
        };
        let (templates, next_page, page_size) = match prefetched {
            Some(page) => (page.templates, page.page + 1, page.size.max(10)),
            // `new WorldTemplatePaginatedList(10)`: page 0 of ten, so page 1 is next.
            None => (Vec::new(), 1, 10),
        };
        let job = client().map(|client| {
            let world_type = group
                .map_or("MINIGAME", TemplateGroup::world_type)
                .to_owned();
            spawn(move || fetch_template_page(&client, world_type.as_str(), next_page, page_size))
        });
        self.subscreen = SubScreen::TemplateList {
            server,
            slot,
            kind,
            creation,
            group,
            templates,
            selected: None,
            scroll: crate::ui::Scroll::default(),
            job,
            select: None,
            icons: std::cell::RefCell::new(Vec::new()),
            error: None,
        };
    }

    fn select_template(&mut self) {
        let t = crate::lang::get;
        let SubScreen::TemplateList {
            server,
            slot,
            kind,
            creation,
            group,
            templates,
            selected,
            ..
        } = &self.subscreen
        else {
            return;
        };
        let Some(template) = selected.and_then(|i| templates.get(i)).cloned() else {
            return;
        };
        let (server, slot, kind, group) = (server.clone(), *slot, *kind, *group);
        let creation = creation.clone();
        if group.is_none() {
            // `SwitchMinigameTask`: `putIntoMinigameMode`, then the configuration screen; the
            // task asks again when the service says no (`for i = 0; i < 25`).
            let task = client().map(|client| {
                let (realm_id, minigame) = (server.id, template.id.clone());
                let title = Arc::new(Mutex::new(t("mco.minigame.world.starting.screen.title")));
                let abort = Arc::new(AtomicBool::new(false));
                let job = {
                    let abort = Arc::clone(&abort);
                    spawn(move || {
                        for _ in 0..25 {
                            if abort.load(Ordering::Relaxed) {
                                return Ok(());
                            }
                            if client.put_into_minigame_mode(realm_id, &minigame)? {
                                break;
                            }
                        }
                        Ok(())
                    })
                };
                RunningTask { job, title, abort }
            });
            if let SubScreen::TemplateList { select, .. } = &mut self.subscreen {
                *select = task;
            }
            return;
        }
        // `ResettingTemplateWorldTask`: the reset screen again, `runResetTasks`' task list
        // running in a `RealmsLongRunningMcoTaskScreen` whose title follows each task.
        let first = creation
            .as_ref()
            .map_or_else(|| t(kind.task_title()), |_| t("mco.create.world.wait"));
        let task = client().map(|client| {
            let (realm_id, active, template_id) = (server.id, server.active_slot, template.id);
            let title = Arc::new(Mutex::new(first));
            let abort = Arc::new(AtomicBool::new(false));
            let job = {
                let (title, abort) = (Arc::clone(&title), Arc::clone(&abort));
                let creation = creation.clone();
                spawn(move || {
                    // `runResetTasks`: `RealmCreationTask` runs before the slot switch and
                    // the reset, then `SwitchSlotTask` when the slot is not the active one.
                    if let Some((name, description)) = creation.as_ref() {
                        client.initialize_realm(realm_id, name, description)?;
                    }
                    if abort.load(Ordering::Relaxed) {
                        return Ok(());
                    }
                    if slot != active {
                        *title.lock().expect("a task title") =
                            t("mco.minigame.world.slot.screen.title");
                        client.switch_slot(realm_id, slot)?;
                        if abort.load(Ordering::Relaxed) {
                            return Ok(());
                        }
                    }
                    *title.lock().expect("a task title") = t(kind.task_title());
                    let template_id = template_id.parse::<i64>().map_err(|_| RealmsError::Parse)?;
                    client
                        .reset_world_with_template(realm_id, template_id)
                        .map(drop)
                })
            };
            RunningTask { job, title, abort }
        });
        self.subscreen = SubScreen::ResetWorld {
            server,
            slot,
            kind,
            creation: creation.clone(),
            templates: None,
            job: None,
            reset: task,
            error: None,
        };
    }

    /// `RealmsCreateWorldFlow.createWorld`: the Create New World screen's settings become
    /// a temporary world (`createTemporaryWorldFolder`) that is packed and uploaded into
    /// the slot; `AlertScreen` (`mco.create.world.reset.title`) shows while it runs.
    pub fn start_create_world_upload(&mut self, request: CreateWorldRequest, world: CreatedWorld) {
        let progress = Arc::new(upload::UploadProgress::default());
        let job = client().map(|client| {
            let progress = Arc::clone(&progress);
            let server = request.server.clone();
            let slot = request.slot;
            let creation = request.creation.clone();
            spawn(move || {
                upload_created_world(&client, &server, slot, creation.as_ref(), &world, &progress)
            })
        });
        if job.is_none() {
            // The upload cannot run without the account's session: the same failure the
            // service's refusal would show, back to the reset screen.
            self.subscreen = SubScreen::Error {
                title: crate::lang::get("mco.errorMessage.generic"),
                detail: crate::lang::get("mco.create.world.failed"),
                next: Box::new(Self::reset_screen(request)),
            };
            return;
        }
        self.subscreen = SubScreen::CreateWorldUpload {
            request,
            progress,
            job,
        };
    }

    /// The Create New World screen's Back for the Generate New World tile
    /// (`CreateWorldScreen.openFresh`'s cancel): the screen the reset screen was opened
    /// from — the configuration screen, or the list for a Realm being created.
    pub fn cancel_create_world(&mut self, request: CreateWorldRequest) {
        match request.kind {
            ResetKind::NewRealm => {
                self.subscreen = SubScreen::Main;
                self.selected = None;
                self.refresh();
            }
            ResetKind::Reset | ResetKind::Empty => self.back_to_configure_worlds(request.server),
        }
    }

    /// `RealmsResetWorldScreen` again, as the reset's own tasks return to it.
    pub(crate) fn reset_screen(request: CreateWorldRequest) -> SubScreen {
        SubScreen::ResetWorld {
            server: request.server,
            slot: request.slot,
            kind: request.kind,
            creation: request.creation,
            templates: None,
            job: None,
            reset: None,
            error: None,
        }
    }

    /// `RealmsMainScreen.play(server, returnScreen, true)` for a Realm that was just
    /// initialized: the address request without the list's Play gate (the list has not
    /// reloaded the Realm yet).
    pub(crate) fn start_join_after_create(&mut self, server: RealmServer) -> Outcome {
        let last = Box::new(self.subscreen.clone());
        self.start_join_from(server, last)
    }
}
