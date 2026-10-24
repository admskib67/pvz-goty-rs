//! Dialog boxes: the stone-framed `LawnDialog` with its header, wrapped
//! text and stone buttons, and the dialogs built on it (new user, who are
//! you?). Every dialog is modal: while one is open, only the topmost dialog
//! gets the mouse and keyboard, and the screen behind gets neither.
//!
//! What a button does is up to the app (`LawnApp::ButtonDepress`), so
//! clicks are queued as [`DialogEvent`]s for `lawn_app` to handle.
//!
//! Behaviour per ruslan831 decomp: `SexyAppFramework/Dialog.cpp`,
//! `Lawn/Widget/LawnDialog.cpp`, `NewUserDialog.cpp`, `UserDialog.cpp`, and
//! `SexyAppBase`'s dialog list (`DoDialog`, `AddDialog`, `KillDialog`).

use bevy::input::ButtonState;
use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::prelude::*;

use crate::audio::Sounds;
use crate::graphics::{Graphics, SexyColor};
use crate::input::GameCursor;
use crate::resources::GameResources;
use crate::widgets::{EditKey, EditWidget, ListColors, ListWidget, StoneButton, draw_edit_box};
use crate::{GAME_HEIGHT, GAME_WIDTH, UiSet};

pub struct DialogPlugin;

impl Plugin for DialogPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Dialogs>()
            .add_systems(FixedUpdate, update_dialogs)
            .add_systems(Update, dialog_input.in_set(UiSet::DialogInput))
            .add_systems(Update, draw_dialogs.in_set(UiSet::DialogDraw));
    }
}

/// `Dialogs`: which dialog this is. The app decides what its buttons do by this.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DialogId {
    Quit,
    UserDialog,
    CreateUser,
    ConfirmDeleteUser,
    RenameUser,
    CreateUserError,
    RenameUserError,
    Message,
    ConfirmBackToMain,
    GameOver,
    /// Crazy Dave's offer of an extra seed slot.
    PurchasePacketSlot,
}

/// `Dialog::BUTTONS_*`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ButtonMode {
    YesNo,
    OkCancel,
    /// One button labelled with the footer text.
    Footer,
}

/// A dialog's buttons. `Yes` is also OK and the footer button (`ID_YES`,
/// `ID_OK` and `ID_FOOTER` are all 1000); `No` is also Cancel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DialogButton {
    Yes,
    No,
    /// `UserDialog_RenameUser`.
    Rename,
    /// `UserDialog_DeleteUser`.
    Delete,
}

/// Something the player did in a dialog, for the app to act on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DialogEvent {
    /// A button was clicked (or its key pressed).
    Button(DialogId, DialogButton),
    /// `ListListener::ListClicked`: a line of a dialog's list was clicked.
    ListClicked(DialogId, i32, u32),
}

/// `DIALOG_HEADER_OFFSET`: room above the frame for the skull header.
const HEADER_OFFSET: i32 = 45;
/// The dialog text's colour (`SetColor(0/1, 0xE0BB62)`).
const TEXT_COLOR: SexyColor = SexyColor::rgba(0xE0, 0xBB, 0x62, 255);
const HEADER_FONT: &str = "FONT_DWARVENTODCRAFT24";
const LINES_FONT: &str = "FONT_DWARVENTODCRAFT15";
/// Fonts of a dialog's text box and user list.
const EDIT_FONT: &str = "FONT_BRIANNETOD16";

/// `gUserListWidgetColors`.
const USER_LIST_COLORS: ListColors = ListColors {
    bkg: SexyColor::rgba(23, 24, 35, 255),
    outline: SexyColor::BLACK,
    text: SexyColor::rgba(235, 225, 180, 255),
    hilite: SexyColor::WHITE,
    select: SexyColor::rgba(20, 180, 15, 255),
};

/// `Insets`.
#[derive(Debug, Clone, Copy)]
struct Insets {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}

/// What kind of dialog it is, with the widgets only that kind has.
#[derive(Debug, Clone)]
pub enum DialogKind {
    /// A plain `LawnDialog`: header, text and buttons.
    Message,
    /// `NewUserDialog`: asks for a name, to create or rename a user.
    NewUser { edit: EditWidget },
    /// `UserDialog`: pick, rename or delete a user, or create one.
    User { list: ListWidget, num_users: i32, rename: StoneButton<DialogButton>, delete: StoneButton<DialogButton> },
}

/// A `LawnDialog`. Its widgets' coordinates are relative to its top-left.
#[derive(Debug, Clone)]
pub struct LawnDialog {
    pub id: DialogId,
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    header: String,
    lines: String,
    content_insets: Insets,
    space_after_header: i32,
    line_spacing_offset: i32,
    /// `mButtonHeight`: 24, as the dialog has no button component image.
    button_height: i32,
    tall_bottom: bool,
    vertical_center_text: bool,
    pub yes_button: Option<StoneButton<DialogButton>>,
    pub no_button: Option<StoneButton<DialogButton>>,
    pub kind: DialogKind,
    /// While dragging the dialog, where the mouse holds it.
    drag: Option<IVec2>,
}

fn image_size(res: &GameResources, id: &str) -> (i32, i32) {
    res.image(id).map_or((0, 0), |i| (i.width as i32, i.height as i32))
}

/// Rounds `extra` up to a whole number of `tile`s (0 if it isn't positive).
fn round_up_to_tiles(extra: i32, tile: i32) -> i32 {
    if extra <= 0 {
        0
    } else if tile > 0 && extra % tile != 0 {
        extra + tile - extra % tile
    } else {
        extra
    }
}

impl LawnDialog {
    /// `LawnDialog::LawnDialog`, sized to fit (`CalcSize(0, 0)`). Strings
    /// may be `[KEY]`s to translate.
    pub fn new(res: &GameResources, id: DialogId, header: &str, lines: &str, footer: &str, mode: ButtonMode) -> Self {
        let button = |id, label: &str| Some(StoneButton::new(id, res.translate(label)));
        let (yes_button, no_button) = match mode {
            ButtonMode::YesNo => (button(DialogButton::Yes, "[BUTTON_YES]"), button(DialogButton::No, "[BUTTON_NO]")),
            ButtonMode::OkCancel => {
                (button(DialogButton::Yes, "[BUTTON_OK]"), button(DialogButton::No, "[BUTTON_CANCEL]"))
            }
            ButtonMode::Footer => (button(DialogButton::Yes, footer), None),
        };
        let mut dialog = Self {
            id,
            x: 0,
            y: 0,
            width: 0,
            height: 0,
            header: res.translate(header),
            lines: res.translate(lines),
            content_insets: Insets { left: 36, top: 35, right: 46, bottom: 36 },
            space_after_header: 10,
            line_spacing_offset: 0,
            button_height: 24,
            tall_bottom: false,
            vertical_center_text: true,
            yes_button,
            no_button,
            kind: DialogKind::Message,
            drag: None,
        };
        dialog.calc_size(res, 0, 0);
        dialog
    }

    /// `NewUserDialog::NewUserDialog`: create a user, or rename one.
    pub fn new_user(res: &GameResources, rename: bool) -> Self {
        let (id, header) = if rename {
            (DialogId::RenameUser, "[RENAME_USER]")
        } else {
            (DialogId::CreateUser, "[NEW_USER]")
        };
        let mut dialog = Self::new(res, id, header, "[PLEASE_ENTER_NAME]", "[DIALOG_BUTTON_OK]", ButtonMode::OkCancel);
        dialog.vertical_center_text = false;
        let mut edit = EditWidget::new();
        edit.max_chars = 12;
        edit.max_pixels = 220;
        dialog.kind = DialogKind::NewUser { edit };
        dialog.calc_size(res, 110, 40);
        dialog
    }

    /// `UserDialog::UserDialog`: the current player first and selected, then
    /// the others, then "(Create a New User)" while there's room.
    pub fn user(res: &GameResources, current: Option<&str>, names: &[&str]) -> Self {
        let mut dialog = Self::new(res, DialogId::UserDialog, "[WHO_ARE_YOU]", "", "", ButtonMode::OkCancel);
        dialog.vertical_center_text = false;
        let mut list = ListWidget::new(EDIT_FONT, USER_LIST_COLORS, 24);
        let mut num_users = 0;
        if let Some(current) = current {
            list.select_idx = list.add_line(current.to_owned());
            num_users += 1;
        }
        for &name in names.iter().filter(|&&n| Some(n) != current) {
            list.add_line(name.to_owned());
            num_users += 1;
        }
        if num_users < 8 {
            list.add_line(res.translate("[CREATE_NEW_USER]"));
        }
        let rename = StoneButton::new(DialogButton::Rename, res.translate("[RENAME_BUTTON]"));
        let delete = StoneButton::new(DialogButton::Delete, res.translate("[DELETE_BUTTON]"));
        dialog.kind = DialogKind::User { list, num_users, rename, delete };
        dialog.tall_bottom = true;
        dialog.calc_size(res, 210, 270);
        dialog
    }

    fn bottom_left(&self) -> &'static str {
        if self.tall_bottom { "IMAGE_DIALOG_BIGBOTTOMLEFT" } else { "IMAGE_DIALOG_BOTTOMLEFT" }
    }

    /// `LawnDialog::GetLeft`/`GetTop`/`GetWidth`: the content area.
    fn content_left(&self) -> i32 {
        self.content_insets.left
    }

    fn content_top(&self) -> i32 {
        self.content_insets.top + 99
    }

    fn content_width(&self) -> i32 {
        self.width - self.content_insets.left - self.content_insets.right
    }

    /// `LawnDialog::CalcSize`: fits the frame around the text plus extra room,
    /// in whole tiles of the frame images.
    fn calc_size(&mut self, res: &GameResources, extra_x: i32, extra_y: i32) {
        let insets = self.content_insets;
        let (Some(header_font), Some(lines_font)) = (res.font(HEADER_FONT), res.font(LINES_FONT)) else { return };
        let (header_font, lines_font) = (&header_font.font, &lines_font.font);

        let mut width = insets.left + insets.right + extra_x;
        if !self.header.is_empty() {
            width += header_font.string_width(&self.header);
        }
        let top_mid = image_size(res, "IMAGE_DIALOG_TOPMIDDLE").0;
        let image_width =
            image_size(res, "IMAGE_DIALOG_TOPLEFT").0 + image_size(res, "IMAGE_DIALOG_TOPRIGHT").0 + top_mid;
        if width <= image_width {
            width = image_width;
        } else {
            width = image_width + round_up_to_tiles(width - image_width, top_mid);
        }

        let mut height = insets.top + insets.bottom + extra_y + HEADER_OFFSET;
        if !self.header.is_empty() {
            height += header_font.height - header_font.ascent_padding + self.space_after_header;
        }
        if !self.lines.is_empty() {
            width += top_mid;
            let wrap_width = width - insets.left - insets.right - 4;
            let spacing = lines_font.line_spacing() + self.line_spacing_offset;
            height += lines_font.word_wrap(&self.lines, wrap_width, spacing).1 + 30;
        }
        height += self.button_height;
        let image_height = image_size(res, "IMAGE_DIALOG_TOPLEFT").1 + image_size(res, self.bottom_left()).1 + HEADER_OFFSET;
        if height < image_height {
            height = image_height;
        } else {
            let center = image_size(res, "IMAGE_DIALOG_CENTERLEFT").1;
            height = image_height + round_up_to_tiles(height - image_height, center);
        }
        self.resize(res, self.x, self.y, width, height);
    }

    /// `LawnDialog::Resize` (and each kind's): lays out the buttons along
    /// the bottom, and the kind's widgets.
    fn resize(&mut self, res: &GameResources, x: i32, y: i32, width: i32, height: i32) {
        (self.x, self.y, self.width, self.height) = (x, y, width, height);
        let insets = self.content_insets;
        let (left_w, button_h) = image_size(res, "IMAGE_BUTTON_LEFT");
        let min_width = left_w + image_size(res, "IMAGE_BUTTON_RIGHT").0;
        let mid_width = image_size(res, "IMAGE_BUTTON_MIDDLE").0;

        let area_x = insets.left - 5;
        let mut area_y = height - insets.bottom - button_h + 2;
        let area_width = width - insets.right - insets.left + 8;
        if self.tall_bottom {
            area_y += 5;
        }
        match (&mut self.yes_button, &mut self.no_button) {
            (Some(yes), Some(no)) => {
                let w = min_width + round_up_to_tiles((area_width - 10) / 2 - mid_width - min_width + 1, mid_width);
                yes.resize(area_x, area_y, w, button_h);
                no.resize(area_width - w + area_x, area_y, w, button_h);
            }
            (Some(yes), None) => {
                let w = min_width + round_up_to_tiles(area_width - mid_width - min_width + 1, mid_width);
                yes.resize(area_x + (area_width - w) / 2, area_y, w, button_h);
            }
            _ => {}
        }

        let (content_left, content_top, content_width) = (self.content_left(), self.content_top(), self.content_width());
        match &mut self.kind {
            DialogKind::Message => {}
            DialogKind::NewUser { edit } => {
                if let Some(font) = res.font(EDIT_FONT) {
                    let w = width - insets.left - insets.right - 24;
                    edit.resize(insets.left + 12, height - 155, w, 28, &font.font);
                }
            }
            DialogKind::User { list, rename, delete, .. } => {
                list.resize(content_left + 30, content_top + 4, content_width - 60, 200);
                // Layout(LAY_SameLeft | LAY_Above | LAY_SameHeight | LAY_SameWidth).
                for (button, below) in [(rename, &self.yes_button), (delete, &self.no_button)] {
                    if let Some(b) = below {
                        button.resize(b.x, b.y - b.height, b.width, b.height);
                    }
                }
            }
        }
    }

    /// `LawnApp::CenterDialog`.
    pub fn center(&mut self, res: &GameResources) {
        let (w, h) = (self.width, self.height);
        self.resize(res, (GAME_WIDTH as i32 - w) / 2, (GAME_HEIGHT as i32 - h) / 2, w, h);
    }

    /// `Dialog::MouseDrag`: moves the dialog with the mouse, keeping it on screen.
    fn drag_to(&mut self, p: IVec2) {
        let Some(hold) = self.drag else { return };
        let (w, h) = (self.width, self.height);
        let (screen_w, screen_h) = (GAME_WIDTH as i32, GAME_HEIGHT as i32);
        let new_x = (p.x - hold.x).max(-8).min(screen_w - w + 8);
        let new_y = (p.y - hold.y).max(-8).min(screen_h - h + 8);
        let hold = IVec2::new((p.x - new_x).clamp(8, w - 9), (p.y - new_y).clamp(8, h - 9));
        self.drag = Some(hold);
        (self.x, self.y) = (new_x, new_y);
    }

    /// `NewUserDialog::GetName`: the typed name with runs of spaces collapsed
    /// and no leading or trailing space.
    pub fn typed_name(&self) -> String {
        let DialogKind::NewUser { edit } = &self.kind else { return String::new() };
        let mut name = String::new();
        let mut last = ' ';
        for &c in &edit.text {
            if c != ' ' || c != last {
                name.push(c);
            }
            last = c;
        }
        if name.ends_with(' ') {
            name.pop();
        }
        name
    }

    /// `NewUserDialog::SetName`.
    pub fn set_typed_name(&mut self, res: &GameResources, name: &str) {
        if let (DialogKind::NewUser { edit }, Some(font)) = (&mut self.kind, res.font(EDIT_FONT)) {
            edit.set_text(name, &font.font);
        }
    }

    /// `UserDialog::GetSelName`: the selected user, or "" for none or the
    /// "create" line.
    pub fn selected_name(&self) -> String {
        match &self.kind {
            DialogKind::User { list, num_users, .. } if (0..*num_users).contains(&list.select_idx) => {
                list.lines[list.select_idx as usize].clone()
            }
            _ => String::new(),
        }
    }

    /// `UserDialog::FinishDeleteUser`: drops the selected line.
    pub fn finish_delete_user(&mut self, res: &GameResources) {
        let DialogKind::User { list, num_users, .. } = &mut self.kind else { return };
        let selected = list.select_idx;
        list.remove_line(selected);
        if !list.lines.is_empty() {
            list.select_idx = (selected - 1).max(0);
        }
        *num_users -= 1;
        if *num_users == 7 {
            list.add_line(res.translate("[CREATE_NEW_USER]"));
        }
    }

    /// `UserDialog::FinishRenameUser`.
    pub fn finish_rename_user(&mut self, new_name: &str) {
        if let DialogKind::User { list, num_users, .. } = &mut self.kind
            && list.select_idx < *num_users
            && list.select_idx >= 0
        {
            list.lines[list.select_idx as usize] = new_name.to_owned();
        }
    }

    /// Selects a line of the user list (`ListWidget::SetSelect`).
    pub fn select_line(&mut self, idx: i32) {
        if let DialogKind::User { list, .. } = &mut self.kind {
            list.select_idx = idx;
        }
    }

    /// The user list's index of the "(Create a New User)" line.
    pub fn create_user_line(&self) -> Option<i32> {
        match &self.kind {
            DialogKind::User { num_users, .. } => Some(*num_users),
            _ => None,
        }
    }

    /// The stone buttons, back to front.
    fn buttons(&self) -> impl Iterator<Item = &StoneButton<DialogButton>> {
        let extra = match &self.kind {
            DialogKind::User { delete, rename, .. } => [Some(delete), Some(rename)],
            _ => [None, None],
        };
        self.yes_button.iter().chain(self.no_button.iter()).chain(extra.into_iter().flatten())
    }

    fn buttons_mut(&mut self) -> impl Iterator<Item = &mut StoneButton<DialogButton>> {
        let extra = match &mut self.kind {
            DialogKind::User { delete, rename, .. } => [Some(delete), Some(rename)],
            _ => [None, None],
        };
        self.yes_button.iter_mut().chain(self.no_button.iter_mut()).chain(extra.into_iter().flatten())
    }

    /// The widget under a point local to the dialog, front to back.
    fn target_at(&self, local: IVec2) -> Option<Target> {
        if let Some(b) = self.buttons().collect::<Vec<_>>().into_iter().rev().find(|b| b.contains(local)) {
            return Some(Target::Button(b.id));
        }
        match &self.kind {
            DialogKind::NewUser { edit } if edit.contains(local) => return Some(Target::Edit),
            DialogKind::User { list, .. } if list.contains(local) => return Some(Target::List),
            _ => {}
        }
        let inside = local.x >= 0 && local.y >= 0 && local.x < self.width && local.y < self.height;
        inside.then_some(Target::Body)
    }

    /// `LawnDialog::Draw` and the dialog's widgets.
    fn draw(&self, g: &mut Graphics, res: &GameResources) {
        g.push_state();
        g.translate(self.x as f32, self.y as f32);
        self.draw_frame(g, res);
        if let DialogKind::NewUser { edit } = &self.kind {
            draw_edit_box(g, res, edit);
        }
        for b in self.buttons().take(2) {
            b.draw(g, res);
        }
        match &self.kind {
            DialogKind::Message => {}
            DialogKind::NewUser { edit } => {
                if let Some(font) = res.font(EDIT_FONT) {
                    edit.draw(g, font);
                }
            }
            DialogKind::User { list, delete, rename, .. } => {
                list.draw(g, res);
                delete.draw(g, res);
                rename.draw(g, res);
            }
        }
        g.pop_state();
    }

    /// `LawnDialog::Draw`: the tiled stone frame, the skull header, the
    /// header text and the wrapped lines.
    fn draw_frame(&self, g: &mut Graphics, res: &GameResources) {
        let image = |id: &str| res.image(id);
        let suffixes = if self.tall_bottom {
            ["IMAGE_DIALOG_BIGBOTTOMLEFT", "IMAGE_DIALOG_BIGBOTTOMMIDDLE", "IMAGE_DIALOG_BIGBOTTOMRIGHT"]
        } else {
            ["IMAGE_DIALOG_BOTTOMLEFT", "IMAGE_DIALOG_BOTTOMMIDDLE", "IMAGE_DIALOG_BOTTOMRIGHT"]
        };
        let (
            Some(top_left),
            Some(top_mid),
            Some(top_right),
            Some(center_left),
            Some(center_mid),
            Some(center_right),
            Some(bottom_left),
            Some(bottom_mid),
            Some(bottom_right),
            Some(header),
        ) = (
            image("IMAGE_DIALOG_TOPLEFT"),
            image("IMAGE_DIALOG_TOPMIDDLE"),
            image("IMAGE_DIALOG_TOPRIGHT"),
            image("IMAGE_DIALOG_CENTERLEFT"),
            image("IMAGE_DIALOG_CENTERMIDDLE"),
            image("IMAGE_DIALOG_CENTERRIGHT"),
            image(suffixes[0]),
            image(suffixes[1]),
            image(suffixes[2]),
            image("IMAGE_DIALOG_HEADER"),
        )
        else {
            return;
        };
        let w = |i: &crate::resources::ImageRes| i.width as i32;
        let h = |i: &crate::resources::ImageRes| i.height as i32;
        let repeat_x = (self.width - w(top_right) - w(top_left)) / w(top_mid).max(1);
        let repeat_y =
            (self.height - h(top_left) - h(bottom_left) - HEADER_OFFSET) / h(center_left).max(1);

        let row = |g: &mut Graphics, y: i32, [left, mid, right]: [&crate::resources::ImageRes; 3]| {
            let mut x = 0;
            g.draw_image(left, x as f32, y as f32);
            x += w(left);
            for _ in 0..repeat_x {
                g.draw_image(mid, x as f32, y as f32);
                x += w(mid);
            }
            g.draw_image(right, x as f32, y as f32);
        };
        let mut y = HEADER_OFFSET;
        row(g, y, [top_left, top_mid, top_right]);
        y += h(top_right);
        for _ in 0..repeat_y {
            row(g, y, [center_left, center_mid, center_right]);
            y += h(center_left);
        }
        row(g, y, [bottom_left, bottom_mid, bottom_right]);
        g.draw_image(header, ((self.width - w(header)) / 2 - 5) as f32, 0.0);

        let (Some(header_font), Some(lines_font)) = (res.font(HEADER_FONT), res.font(LINES_FONT)) else { return };
        let insets = self.content_insets;
        let mut font_y = insets.top + HEADER_OFFSET;
        g.push_state();
        if !self.header.is_empty() {
            let hf = &header_font.font;
            let baseline = font_y - hf.ascent_padding + hf.ascent;
            g.set_color(TEXT_COLOR);
            g.write_centered_line(header_font, self.width, baseline, &self.header);
            font_y = baseline - hf.ascent + hf.height + self.space_after_header;
        }

        let lf = &lines_font.font;
        let spacing = lf.line_spacing() + self.line_spacing_offset;
        let area_width = self.width - insets.left - insets.right - 4;
        let mut lines_y = font_y;
        if self.vertical_center_text {
            let lines_height = lf.word_wrap(&self.lines, area_width, spacing).1;
            let mut area_height = self.height - insets.bottom - self.button_height - font_y - 55;
            if self.tall_bottom {
                area_height -= 36;
            }
            lines_y += (area_height - lines_height) / 2;
        }
        g.set_color(TEXT_COLOR);
        g.write_word_wrapped(lines_font, insets.left + 2, lines_y, area_width, &self.lines, spacing, 0);
        g.pop_state();
    }
}

/// A widget in a dialog that the mouse can be over.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Target {
    Button(DialogButton),
    Edit,
    List,
    /// The dialog itself: dragging it moves it.
    Body,
}

/// The open dialogs, back to front (`mDialogList`), and the events their
/// buttons produced this frame.
#[derive(Resource, Default)]
pub struct Dialogs {
    list: Vec<LawnDialog>,
    events: Vec<DialogEvent>,
    /// What the held mouse button went down on (`mLastDownWidget`).
    pressed: Option<(DialogId, Target)>,
    last_click: Option<(f64, Vec2)>,
    click_count: u32,
}

impl Dialogs {
    /// True while any dialog is open: the screen behind gets no input.
    pub fn is_modal(&self) -> bool {
        !self.list.is_empty()
    }

    pub fn get(&self, id: DialogId) -> Option<&LawnDialog> {
        self.list.iter().find(|d| d.id == id)
    }

    pub fn get_mut(&mut self, id: DialogId) -> Option<&mut LawnDialog> {
        self.list.iter_mut().find(|d| d.id == id)
    }

    /// `SexyAppBase::AddDialog`: replaces any dialog with the same id and
    /// puts this one on top.
    pub fn add_dialog(&mut self, dialog: LawnDialog) -> &mut LawnDialog {
        self.kill_dialog(dialog.id);
        self.list.push(dialog);
        self.list.last_mut().expect("just pushed")
    }

    /// `LawnApp::DoDialog`: a centered `LawnDialog`.
    pub fn do_dialog(
        &mut self,
        res: &GameResources,
        id: DialogId,
        header: &str,
        lines: &str,
        footer: &str,
        mode: ButtonMode,
    ) -> &mut LawnDialog {
        let mut dialog = LawnDialog::new(res, id, header, lines, footer, mode);
        dialog.center(res);
        self.add_dialog(dialog)
    }

    /// `KillDialog`. Returns whether it was open.
    pub fn kill_dialog(&mut self, id: DialogId) -> bool {
        let before = self.list.len();
        self.list.retain(|d| d.id != id);
        if self.pressed.is_some_and(|(d, _)| d == id) {
            self.pressed = None;
        }
        self.list.len() != before
    }

    /// Takes this frame's events.
    pub fn take_events(&mut self) -> Vec<DialogEvent> {
        std::mem::take(&mut self.events)
    }
}

/// `Widget::Update` for every dialog: blinks text box cursors. Only the
/// topmost dialog's text box has focus.
fn update_dialogs(mut dialogs: ResMut<Dialogs>) {
    let top = dialogs.list.len().checked_sub(1);
    for (i, dialog) in dialogs.list.iter_mut().enumerate() {
        if let DialogKind::NewUser { edit } = &mut dialog.kind {
            let focused = Some(i) == top;
            if focused && !edit.has_focus {
                edit.got_focus();
            } else if !focused && edit.has_focus {
                edit.lost_focus();
            }
            edit.update();
        }
    }
}

/// How close in time (seconds) two clicks must be to count as a double click.
const DOUBLE_CLICK_TIME: f64 = 0.5;

/// Routes the mouse and keyboard to the topmost dialog (`WidgetManager`
/// with a modal dialog open).
#[allow(clippy::too_many_arguments)]
fn dialog_input(
    mut dialogs: ResMut<Dialogs>,
    cursor: Res<GameCursor>,
    mouse: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    mut key_events: MessageReader<KeyboardInput>,
    time: Res<Time<Real>>,
    res: Option<Res<GameResources>>,
    mut sounds: Sounds,
) {
    let typed: Vec<KeyboardInput> = key_events.read().filter(|k| k.state == ButtonState::Pressed).cloned().collect();
    let Some(res) = res else { return };
    let dialogs = &mut *dialogs;
    if mouse.just_pressed(MouseButton::Left)
        && let Some(p) = cursor.0
    {
        let now = time.elapsed_secs_f64();
        let repeat = dialogs.last_click.is_some_and(|(t, q)| now - t < DOUBLE_CLICK_TIME && q.distance(p) < 4.0);
        dialogs.click_count = if repeat { dialogs.click_count + 1 } else { 1 };
        dialogs.last_click = Some((now, p));
    }
    let click_count = dialogs.click_count;
    let Some(dialog) = dialogs.list.last_mut() else { return };
    let id = dialog.id;
    let edit_font = res.font(EDIT_FONT).map(|f| f.font.clone());
    let p = cursor.0.map(|p| p.floor().as_ivec2());
    let local = p.map(|p| p - IVec2::new(dialog.x, dialog.y));
    let held = dialogs.pressed.filter(|(d, _)| *d == id).map(|(_, t)| t);
    let hovered = local.and_then(|l| dialog.target_at(l));
    let mut events = Vec::new();

    // Hover: while the mouse is held, only the widget it went down on reacts.
    for b in dialog.buttons_mut() {
        let over = local.is_some_and(|l| b.contains(l));
        b.is_over = match held {
            Some(Target::Button(h)) => h == b.id && over,
            Some(_) => false,
            None => hovered == Some(Target::Button(b.id)),
        };
    }
    if let DialogKind::User { list, .. } = &mut dialog.kind {
        let over = held.is_none() && hovered == Some(Target::List);
        list.mouse_move(local.filter(|_| over).map(|l| l - IVec2::new(list.x, list.y)));
    }

    if mouse.just_pressed(MouseButton::Left)
        && let Some(local) = local
    {
        let target = hovered;
        match target {
            Some(Target::Button(b)) => {
                if let Some(button) = dialog.buttons_mut().find(|x| x.id == b) {
                    button.is_down = true;
                }
                // LawnDialog::ButtonPress.
                sounds.play_sample("SOUND_GRAVEBUTTON");
            }
            Some(Target::Edit) => {
                if let (DialogKind::NewUser { edit }, Some(font)) = (&mut dialog.kind, &edit_font) {
                    let at = local - IVec2::new(edit.x, edit.y);
                    edit.mouse_down(at, click_count, font);
                }
            }
            Some(Target::List) => {
                if let DialogKind::User { list, .. } = &dialog.kind
                    && list.hilite_idx != -1
                {
                    events.push(DialogEvent::ListClicked(id, list.hilite_idx, click_count));
                }
            }
            Some(Target::Body) if click_count == 1 => dialog.drag = Some(local),
            _ => {}
        }
        dialogs.pressed = target.map(|t| (id, t));
    } else if mouse.pressed(MouseButton::Left)
        && let (Some(p), Some(local), Some(target)) = (p, local, held)
    {
        match target {
            Target::Edit => {
                if let (DialogKind::NewUser { edit }, Some(font)) = (&mut dialog.kind, &edit_font) {
                    let at = local - IVec2::new(edit.x, edit.y);
                    edit.mouse_drag(at, font);
                }
            }
            Target::Body => dialog.drag_to(p),
            _ => {}
        }
    }
    if mouse.just_released(MouseButton::Left) {
        match held {
            Some(Target::Button(b)) => {
                if let Some(button) = dialog.buttons_mut().find(|x| x.id == b) {
                    button.is_down = false;
                    if button.is_over && !button.disabled {
                        events.push(DialogEvent::Button(id, b));
                    }
                }
            }
            Some(Target::Edit) => {
                if let (DialogKind::NewUser { edit }, Some(font), Some(local)) = (&mut dialog.kind, &edit_font, local) {
                    let at = local - IVec2::new(edit.x, edit.y);
                    edit.mouse_up(at, font);
                }
            }
            _ => {}
        }
        dialog.drag = None;
        dialogs.pressed = None;
    }

    // Keyboard: the text box if there is one (LawnEditWidget), else the
    // dialog (LawnDialog::KeyDown).
    let shift = keys.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight]);
    let ctrl = keys.any_pressed([KeyCode::ControlLeft, KeyCode::ControlRight]);
    let has_no = dialog.no_button.is_some();
    for k in typed {
        match &mut dialog.kind {
            DialogKind::NewUser { edit } => {
                let Some(font) = &edit_font else { continue };
                let key = match &k.logical_key {
                    Key::ArrowLeft => Some(EditKey::Left),
                    Key::ArrowRight => Some(EditKey::Right),
                    Key::Backspace => Some(EditKey::Backspace),
                    Key::Delete => Some(EditKey::Delete),
                    Key::Home => Some(EditKey::Home),
                    Key::End => Some(EditKey::End),
                    Key::Enter => Some(EditKey::Return),
                    Key::Escape => {
                        if has_no {
                            events.push(DialogEvent::Button(id, DialogButton::No));
                        }
                        continue;
                    }
                    _ => None,
                };
                match key {
                    // EditWidgetText: Return is the OK button.
                    Some(key) => {
                        if edit.key_down(key, shift, ctrl, font) {
                            events.push(DialogEvent::Button(id, DialogButton::Yes));
                        }
                    }
                    None => {
                        for c in k.text.iter().flat_map(|t| t.chars()).filter(|c| !c.is_control()) {
                            // NewUserDialog::AllowChar.
                            edit.key_char(c, font, |c| c.is_ascii_alphanumeric() || c == ' ');
                        }
                    }
                }
            }
            _ => {
                let typed_char = k.text.as_ref().and_then(|t| t.chars().next());
                let yes = matches!(k.logical_key, Key::Space | Key::Enter) || matches!(typed_char, Some('y' | 'Y'));
                let no = matches!(k.logical_key, Key::Escape) || matches!(typed_char, Some('n' | 'N'));
                if yes {
                    events.push(DialogEvent::Button(id, DialogButton::Yes));
                } else if no && has_no {
                    events.push(DialogEvent::Button(id, DialogButton::No));
                }
            }
        }
    }
    dialogs.events.extend(events);
}

fn draw_dialogs(dialogs: Res<Dialogs>, res: Option<Res<GameResources>>, mut g: ResMut<Graphics>) {
    let Some(res) = res else { return };
    for dialog in &dialogs.list {
        dialog.draw(&mut g, &res);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rounds_extra_size_up_to_whole_tiles() {
        assert_eq!(round_up_to_tiles(-3, 10), 0);
        assert_eq!(round_up_to_tiles(20, 10), 20);
        assert_eq!(round_up_to_tiles(21, 10), 30);
        assert_eq!(round_up_to_tiles(5, 0), 5);
    }

    #[test]
    fn typed_names_collapse_spaces() {
        let mut dialog = LawnDialog {
            id: DialogId::CreateUser,
            x: 0,
            y: 0,
            width: 0,
            height: 0,
            header: String::new(),
            lines: String::new(),
            content_insets: Insets { left: 0, top: 0, right: 0, bottom: 0 },
            space_after_header: 0,
            line_spacing_offset: 0,
            button_height: 0,
            tall_bottom: false,
            vertical_center_text: false,
            yes_button: None,
            no_button: None,
            kind: DialogKind::NewUser { edit: EditWidget::new() },
            drag: None,
        };
        if let DialogKind::NewUser { edit } = &mut dialog.kind {
            edit.text = "  Ann   Lee ".chars().collect();
        }
        assert_eq!(dialog.typed_name(), "Ann Lee");
    }
}
