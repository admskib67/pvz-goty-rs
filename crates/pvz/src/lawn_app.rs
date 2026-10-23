//! The app-level parts of `LawnApp`: opening the standard dialogs and
//! acting on their buttons (`ButtonDepress` and the `Finish…Dialog`
//! functions), and choosing, saving and switching the current player.
//!
//! Behaviour per ruslan831 decomp, `LawnApp.cpp`: `Init` (picking the
//! player), `LawnMessageBox`, `ConfirmQuit`, `DoUserDialog`,
//! `DoCreateUserDialog`, `DoRenameUserDialog`, `DoConfirmDeleteUserDialog`,
//! their `Finish…` counterparts, `FinishNameError`, `ButtonDepress` and
//! `WriteToRegistry`.

use bevy::prelude::*;

use crate::dialog::{ButtonMode, DialogButton, DialogEvent, DialogId, Dialogs, LawnDialog};
use crate::install::{self, Config};
use crate::player::ProfileMgr;
use crate::resources::GameResources;
use crate::{AppState, UiSet};

pub struct LawnAppPlugin;

impl Plugin for LawnAppPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<PlayerChanged>()
            .add_systems(Startup, load_profiles)
            .add_systems(Update, handle_dialog_events.in_set(UiSet::DialogActions))
            .add_systems(Last, save_on_exit);
    }
}

/// The current player changed (or was renamed): screens showing the
/// player re-read it (`GameSelector::SyncProfile`).
#[derive(Message, Debug, Clone, Copy)]
pub struct PlayerChanged;

/// `LawnApp::Init`: loads the roster and picks the last player (the
/// registry's `CurUser`), or any player.
fn load_profiles(mut commands: Commands) {
    let mut profiles = ProfileMgr::load(install::userdata_dir());
    let current = Config::load().cur_user.and_then(|name| profiles.get_profile(&name)).or_else(|| profiles.get_any_profile());
    profiles.set_current(current.as_deref());
    commands.insert_resource(profiles);
}

/// Makes `name` the current player (`mPlayerInfo = …`) and remembers it
/// for next time (`WriteToRegistry`'s `CurUser`).
fn set_player(profiles: &mut ProfileMgr, name: Option<&str>, changed: &mut MessageWriter<PlayerChanged>) {
    profiles.set_current(name);
    let mut config = Config::load();
    config.cur_user = profiles.player().map(|p| p.name.clone());
    if let Err(e) = config.save() {
        error!("couldn't save the current player: {e}");
    }
    changed.write(PlayerChanged);
}

/// `LawnApp::WriteToRegistry` on the way out: saves the player's progress.
fn save_on_exit(mut exit: MessageReader<AppExit>, profiles: Option<Res<ProfileMgr>>) {
    if exit.read().next().is_some()
        && let Some(profiles) = profiles
    {
        profiles.save_current_details();
    }
}

/// `LawnApp::LawnMessageBox`: a message with up to two buttons, labelled
/// with translated `button1`/`button2`. The original waits for the answer;
/// here the dialog's buttons report through [`DialogEvent`]s, and buttons
/// with no special meaning just close it.
#[allow(clippy::too_many_arguments)]
pub fn lawn_message_box(
    dialogs: &mut Dialogs,
    res: &GameResources,
    id: DialogId,
    header: &str,
    lines: &str,
    button1: &str,
    button2: &str,
    mode: ButtonMode,
) {
    let dialog = dialogs.do_dialog(res, id, header, lines, button1, mode);
    if let Some(yes) = &mut dialog.yes_button {
        yes.label = res.translate(button1);
    }
    if let Some(no) = &mut dialog.no_button {
        no.label = res.translate(button2);
    }
}

/// `LawnApp::ConfirmQuit`.
pub fn confirm_quit(dialogs: &mut Dialogs, res: &GameResources) {
    let dialog = dialogs.do_dialog(res, DialogId::Quit, "[QUIT_HEADER]", "[QUIT_MESSAGE]", "", ButtonMode::OkCancel);
    if let Some(yes) = &mut dialog.yes_button {
        yes.label = res.translate("[QUIT_BUTTON]");
    }
}

/// `LawnApp::DoUserDialog`: "Who are you?".
pub fn do_user_dialog(dialogs: &mut Dialogs, res: &GameResources, profiles: &ProfileMgr) {
    let names: Vec<&str> = profiles.names().collect();
    let mut dialog = LawnDialog::user(res, profiles.player().map(|p| p.name.as_str()), &names);
    dialog.center(res);
    dialogs.add_dialog(dialog);
}

/// `LawnApp::DoCreateUserDialog`.
pub fn do_create_user_dialog(dialogs: &mut Dialogs, res: &GameResources) {
    let mut dialog = LawnDialog::new_user(res, false);
    dialog.center(res);
    dialogs.add_dialog(dialog);
}

/// `LawnApp::DoRenameUserDialog`.
fn do_rename_user_dialog(dialogs: &mut Dialogs, res: &GameResources, name: &str) {
    let mut dialog = LawnDialog::new_user(res, true);
    dialog.center(res);
    dialog.set_typed_name(res, name);
    dialogs.add_dialog(dialog);
}

/// `LawnApp::DoConfirmDeleteUserDialog`.
fn do_confirm_delete_user_dialog(dialogs: &mut Dialogs, res: &GameResources, name: &str) {
    let lines = format!("This will permanently remove '{name}' from the player roster!");
    dialogs.do_dialog(res, DialogId::ConfirmDeleteUser, "Are You Sure", &lines, "", ButtonMode::YesNo);
}

/// The "please enter a name" and "name taken" errors.
fn name_error(dialogs: &mut Dialogs, res: &GameResources, id: DialogId, conflict: bool) {
    let (header, lines) = if conflict {
        ("Name Conflict", "The name you entered is already being used.  Please enter a unique player name")
    } else {
        (
            "Enter Your Name",
            "Please enter your name to create a new user profile for storing high score data and game progress",
        )
    };
    dialogs.do_dialog(res, id, header, lines, "OK", ButtonMode::Footer);
}

/// Everything a dialog's buttons can affect.
struct DialogActions<'a, 'c, 'e> {
    dialogs: &'a mut Dialogs,
    res: &'a GameResources,
    profiles: &'a mut ProfileMgr,
    changed: &'a mut MessageWriter<'c, PlayerChanged>,
    exit: &'a mut MessageWriter<'e, AppExit>,
    next_state: &'a mut NextState<AppState>,
}

impl DialogActions<'_, '_, '_> {
    /// `LawnApp::FinishUserDialog`.
    fn finish_user_dialog(&mut self, yes: bool) {
        let Some(dialog) = self.dialogs.get(DialogId::UserDialog) else { return };
        if yes && let Some(name) = self.profiles.get_profile(&dialog.selected_name()) {
            set_player(self.profiles, Some(&name), self.changed);
        }
        self.dialogs.kill_dialog(DialogId::UserDialog);
    }

    /// `LawnApp::FinishCreateUserDialog`. The first player can't cancel.
    fn finish_create_user_dialog(&mut self, yes: bool) {
        let Some(dialog) = self.dialogs.get(DialogId::CreateUser) else { return };
        let name = dialog.typed_name();
        if (yes && name.is_empty()) || (self.profiles.player().is_none() && !yes) {
            name_error(self.dialogs, self.res, DialogId::CreateUserError, false);
        } else if !yes {
            self.dialogs.kill_dialog(DialogId::CreateUser);
        } else if !self.profiles.add_profile(&name) {
            name_error(self.dialogs, self.res, DialogId::CreateUserError, true);
        } else {
            self.profiles.save();
            set_player(self.profiles, Some(&name), self.changed);
            self.dialogs.kill_dialog(DialogId::UserDialog);
            self.dialogs.kill_dialog(DialogId::CreateUser);
        }
    }

    /// `LawnApp::FinishConfirmDeleteUserDialog`.
    fn finish_confirm_delete_user_dialog(&mut self, yes: bool) {
        self.dialogs.kill_dialog(DialogId::ConfirmDeleteUser);
        let Some(dialog) = self.dialogs.get_mut(DialogId::UserDialog) else { return };
        if !yes {
            return;
        }
        let name = dialog.selected_name();
        self.profiles.delete_profile(&name);
        dialog.finish_delete_user(self.res);
        let selected = dialog.selected_name();
        if self.profiles.player().is_none() {
            let next = self.profiles.get_profile(&selected).or_else(|| self.profiles.get_any_profile());
            set_player(self.profiles, next.as_deref(), self.changed);
        }
        self.profiles.save();
        if self.profiles.player().is_none() {
            do_create_user_dialog(self.dialogs, self.res);
        }
        self.changed.write(PlayerChanged);
    }

    /// `LawnApp::FinishRenameUserDialog`.
    fn finish_rename_user_dialog(&mut self, yes: bool) {
        if !yes {
            self.dialogs.kill_dialog(DialogId::RenameUser);
            return;
        }
        let (Some(user), Some(rename)) = (self.dialogs.get(DialogId::UserDialog), self.dialogs.get(DialogId::RenameUser))
        else {
            return;
        };
        let (old_name, new_name) = (user.selected_name(), rename.typed_name());
        if new_name.is_empty() {
            return;
        }
        if !self.profiles.rename_profile(&old_name, &new_name) {
            name_error(self.dialogs, self.res, DialogId::RenameUserError, true);
            return;
        }
        self.profiles.save();
        // The current player keeps its id, so it stays current; just
        // remember its new name.
        let current = self.profiles.player().map(|p| p.name.clone());
        set_player(self.profiles, current.as_deref(), self.changed);
        if let Some(user) = self.dialogs.get_mut(DialogId::UserDialog) {
            user.finish_rename_user(&new_name);
        }
        self.dialogs.kill_dialog(DialogId::RenameUser);
    }

    /// `LawnApp::ButtonDepress` for a dialog's Yes/OK (2000 + id) or
    /// No/Cancel (3000 + id) button.
    fn button_depress(&mut self, id: DialogId, yes: bool) {
        match id {
            DialogId::Quit if yes => {
                self.dialogs.kill_dialog(DialogId::Quit);
                self.exit.write(AppExit::Success);
            }
            DialogId::ConfirmBackToMain if yes => {
                self.dialogs.kill_dialog(DialogId::ConfirmBackToMain);
                self.do_back_to_main();
            }
            DialogId::UserDialog => self.finish_user_dialog(yes),
            DialogId::CreateUser => self.finish_create_user_dialog(yes),
            DialogId::ConfirmDeleteUser => self.finish_confirm_delete_user_dialog(yes),
            DialogId::RenameUser => self.finish_rename_user_dialog(yes),
            // FinishNameError: back to typing the name.
            _ => {
                self.dialogs.kill_dialog(id);
            }
        }
    }

    /// `LawnApp::DoBackToMain`: leaves the level for the main menu. Leaving
    /// the board state stops its music and clears it away (`KillBoard`).
    /// TODO: save the game in progress (`Board::TryToSaveGame`).
    fn do_back_to_main(&mut self) {
        self.profiles.save_current_details();
        self.next_state.set(AppState::GameSelector);
    }

    /// `UserDialog::ButtonDepress` for Rename and Delete.
    fn user_dialog_button(&mut self, button: DialogButton) {
        let Some(name) = self.dialogs.get(DialogId::UserDialog).map(|d| d.selected_name()) else { return };
        if name.is_empty() {
            return;
        }
        match button {
            DialogButton::Rename => do_rename_user_dialog(self.dialogs, self.res, &name),
            DialogButton::Delete => do_confirm_delete_user_dialog(self.dialogs, self.res, &name),
            _ => {}
        }
    }

    /// `UserDialog::ListClicked`: the last line creates a user; a double
    /// click picks one.
    fn user_list_clicked(&mut self, idx: i32, click_count: u32) {
        let Some(dialog) = self.dialogs.get_mut(DialogId::UserDialog) else { return };
        if Some(idx) == dialog.create_user_line() {
            do_create_user_dialog(self.dialogs, self.res);
        } else {
            dialog.select_line(idx);
            if click_count == 2 {
                self.finish_user_dialog(true);
            }
        }
    }
}

fn handle_dialog_events(
    mut dialogs: ResMut<Dialogs>,
    res: Option<Res<GameResources>>,
    profiles: Option<ResMut<ProfileMgr>>,
    mut changed: MessageWriter<PlayerChanged>,
    mut exit: MessageWriter<AppExit>,
    mut next_state: ResMut<NextState<AppState>>,
) {
    let events = dialogs.take_events();
    let (Some(res), Some(mut profiles)) = (res, profiles) else { return };
    let mut app = DialogActions {
        dialogs: &mut dialogs,
        res: &res,
        profiles: &mut profiles,
        changed: &mut changed,
        exit: &mut exit,
        next_state: &mut next_state,
    };
    for event in events {
        match event {
            DialogEvent::Button(id, DialogButton::Yes) => app.button_depress(id, true),
            DialogEvent::Button(id, DialogButton::No) => app.button_depress(id, false),
            DialogEvent::Button(DialogId::UserDialog, b) => app.user_dialog_button(b),
            DialogEvent::ListClicked(DialogId::UserDialog, idx, count) => app.user_list_clicked(idx, count),
            _ => {}
        }
    }
}
