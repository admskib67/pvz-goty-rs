//! The app-level parts of `LawnApp`: opening the standard dialogs and
//! acting on their buttons (`ButtonDepress` and the `Finish…Dialog`
//! functions), and choosing, saving and switching the current player.
//!
//! Behaviour per ruslan831 decomp, `LawnApp.cpp`: `Init` (picking the
//! player), `LawnMessageBox`, `ConfirmQuit`, `DoUserDialog`,
//! `DoCreateUserDialog`, `DoRenameUserDialog`, `DoConfirmDeleteUserDialog`,
//! their `Finish…` counterparts, `FinishNameError`, `ButtonDepress`,
//! `KillNewOptionsDialog`, `NewOptionsDialog::ButtonDepress`/`SliderVal` and
//! `WriteToRegistry`.

use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use crate::audio::{Music, Sounds};
use crate::board::{Board, LoadSavedGame, erase_save, need_save_game, save_dir, try_to_save_game};
use crate::dialog::{ButtonMode, DialogButton, DialogEvent, DialogId, DialogSlider, Dialogs, LawnDialog};
use crate::install::{self, Config};
use crate::options::Settings;
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

/// `LawnApp::WriteToRegistry` on the way out: saves the board in progress
/// (`Board::TryToSaveGame`) and the player's progress.
fn save_on_exit(
    mut exit: MessageReader<AppExit>,
    profiles: Option<Res<ProfileMgr>>,
    board: Option<Res<Board>>,
) {
    if exit.read().next().is_none() {
        return;
    }
    let Some(profiles) = profiles else { return };
    if let (Some(board), Some(dir)) = (board.as_deref(), save_dir()) {
        try_to_save_game(board, &profiles, &dir);
    }
    profiles.save_current_details();
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

/// `LawnApp::DoConfirmBackToMain`.
pub fn do_confirm_back_to_main(dialogs: &mut Dialogs, res: &GameResources) {
    let lines = "Do you want to return\nto the main menu?\n\nYour game will be saved.";
    let dialog = dialogs.do_dialog(res, DialogId::ConfirmBackToMain, "Leave Game?", lines, "", ButtonMode::YesNo);
    if let Some(yes) = &mut dialog.yes_button {
        yes.label = res.translate("[LEAVE_BUTTON]");
    }
    if let Some(no) = &mut dialog.no_button {
        no.label = res.translate("[DIALOG_BUTTON_CANCEL]");
    }
}

/// `LawnApp::DoContinueDialog`: resume the save or start the level over.
pub fn do_continue_dialog(dialogs: &mut Dialogs, res: &GameResources, adventure: bool) {
    let mut dialog = LawnDialog::continue_game(res, adventure);
    dialog.center(res);
    dialogs.add_dialog(dialog);
}

/// `NewOptionsDialog_Restart`'s question. Adventure levels only, so far.
fn do_confirm_restart(dialogs: &mut Dialogs, res: &GameResources) {
    let dialog = dialogs.do_dialog(
        res,
        DialogId::ConfirmRestart,
        "[RESTART_LEVEL_HEADER]",
        "[RESTART_LEVEL_BODY]",
        "",
        ButtonMode::YesNo,
    );
    if let Some(yes) = &mut dialog.yes_button {
        yes.label = res.translate("[RESTART_LABEL]");
    }
    if let Some(no) = &mut dialog.no_button {
        no.label = res.translate("[DIALOG_BUTTON_CANCEL]");
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
    /// The options' Credits button was pressed: show the zombie's note.
    open_credits_note: bool,
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

    /// `CutScene::AdvanceCrazyDaveDialog`'s answer to Dave's seed slot
    /// offer: buying it spends the player's coins.
    fn finish_purchase_packet_slot(&mut self, yes: bool) {
        self.dialogs.kill_dialog(DialogId::PurchasePacketSlot);
        let Some(player) = self.profiles.player_mut() else { return };
        let slot = crate::board::STORE_ITEM_PACKET_UPGRADE;
        if yes {
            player.coins -= crate::board::packet_upgrade_cost(player.purchases[slot]);
            player.purchases[slot] += 1;
        } else {
            player.didnt_purchase_packet_upgrade += 1;
        }
        self.profiles.save_current_details();
    }

    /// `LawnApp::ButtonDepress` for a dialog's Yes/OK (2000 + id) or
    /// No/Cancel (3000 + id) button. `board` saves the level in progress
    /// when leaving it.
    fn button_depress(&mut self, id: DialogId, yes: bool, board: Option<&Board>) {
        match id {
            DialogId::Quit if yes => {
                self.dialogs.kill_dialog(DialogId::Quit);
                self.exit.write(AppExit::Success);
            }
            DialogId::ConfirmBackToMain if yes => {
                self.dialogs.kill_dialog(DialogId::ConfirmBackToMain);
                self.do_back_to_main(board);
            }
            // The level starts over (PreNewGame); leaving and re-entering
            // the board state clears it away and builds it again.
            DialogId::ConfirmRestart if yes => {
                self.dialogs.kill_dialog(DialogId::ConfirmRestart);
                self.dialogs.kill_dialog(DialogId::NewOptions);
                self.next_state.set(AppState::Board);
            }
            DialogId::GameOver => {
                self.dialogs.kill_dialog(DialogId::GameOver);
                if yes {
                    // LawnApp::EndLevel: the same level again.
                    self.next_state.set(AppState::Board);
                } else {
                    self.do_back_to_main(board);
                }
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

    /// `LawnApp::DoBackToMain`: leaves the level for the main menu, saving
    /// the game in progress first (`Board::TryToSaveGame`). Leaving the
    /// board state stops its music and clears it away (`KillBoard`).
    fn do_back_to_main(&mut self, board: Option<&Board>) {
        if let (Some(board), Some(dir)) = (board, save_dir()) {
            try_to_save_game(board, self.profiles, &dir);
        }
        self.dialogs.kill_dialog(DialogId::NewOptions);
        self.profiles.save_current_details();
        // A challenge returns to its own screen, not the main menu.
        if board.is_some_and(crate::board::Board::is_survival) {
            self.next_state.set(AppState::Challenges);
        } else {
            self.next_state.set(AppState::GameSelector);
        }
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

/// `LawnApp::KillNewOptionsDialog`: closes the options, switching the screen
/// mode to what they ask for.
fn kill_new_options_dialog(dialogs: &mut Dialogs, settings: &mut Settings, windows: &mut Query<&mut Window, With<PrimaryWindow>>) {
    let Some(options) = dialogs.get(DialogId::NewOptions).and_then(|d| d.options()) else { return };
    settings.fullscreen = options.fullscreen.checked;
    settings.hardware_acceleration = options.hardware_acceleration.checked;
    for mut window in windows.iter_mut() {
        window.mode = settings.window_mode();
    }
    dialogs.kill_dialog(DialogId::NewOptions);
}

/// The options dialog's sliders, checkboxes and buttons.
#[allow(clippy::too_many_arguments)]
fn options_event(
    event: DialogEvent,
    app: &mut DialogActions,
    settings: &mut Settings,
    music: Option<&mut Music>,
    sounds: &mut Sounds,
    windows: &mut Query<&mut Window, With<PrimaryWindow>>,
    board: Option<&Board>,
) {
    match event {
        // NewOptionsDialog::SliderVal: the volume changes as the thumb moves.
        DialogEvent::SliderVal(_, slider, value, dragging) => {
            settings.set_slider(slider, value);
            match slider {
                DialogSlider::Music => {
                    if let Some(music) = music {
                        music.set_volume(settings.music_volume);
                    }
                }
                DialogSlider::Sfx => {
                    sounds.set_sfx_volume(settings.sfx_volume);
                    if !dragging {
                        sounds.play_sample("SOUND_BUTTONCLICK");
                    }
                }
            }
        }
        // The checkboxes only take effect when the dialog closes. The
        // original's warnings (no windowed mode, unsupported 3D) can't happen.
        DialogEvent::CheckboxChecked(..) => {}
        DialogEvent::Button(_, DialogButton::Yes) => kill_new_options_dialog(app.dialogs, settings, windows),
        // Escape (ID_CANCEL) closes it without switching the screen mode.
        DialogEvent::Button(_, DialogButton::No) => {
            app.dialogs.kill_dialog(DialogId::NewOptions);
        }
        DialogEvent::Button(_, DialogButton::MainMenu) => {
            let from_game_selector =
                app.dialogs.get(DialogId::NewOptions).and_then(|d| d.options()).is_some_and(|o| o.from_game_selector);
            if from_game_selector {
                kill_new_options_dialog(app.dialogs, settings, windows);
                // The options' Credits button: the zombie's note, then the credits.
                app.open_credits_note = true;
                app.next_state.set(AppState::Award);
            } else if board.as_ref().is_some_and(|b| need_save_game(b)) {
                // Board::NeedSaveGame: a level in progress asks first; leaving
                // saves it (TryToSaveGame).
                do_confirm_back_to_main(app.dialogs, app.res);
            } else {
                app.do_back_to_main(board);
            }
        }
        DialogEvent::Button(_, DialogButton::Restart) => do_confirm_restart(app.dialogs, app.res),
        // Over the options, which stay open beneath it.
        DialogEvent::Button(_, DialogButton::Almanac) => {
            crate::almanac::do_almanac_dialog(app.dialogs, app.res, crate::almanac::AlmanacStart::Index)
        }
        DialogEvent::Button(_, button) => info!("options {button:?} isn't ported yet"),
        DialogEvent::ListClicked(..) => {}
    }
}

#[allow(clippy::too_many_arguments)]
fn handle_dialog_events(
    mut commands: Commands,
    mut dialogs: ResMut<Dialogs>,
    res: Option<Res<GameResources>>,
    profiles: Option<ResMut<ProfileMgr>>,
    mut changed: MessageWriter<PlayerChanged>,
    mut exit: MessageWriter<AppExit>,
    mut next_state: ResMut<NextState<AppState>>,
    mut board: Option<ResMut<crate::board::Board>>,
    mut store: Option<ResMut<crate::store::StoreScreen>>,
    mut credits: Option<ResMut<crate::credits::CreditScreen>>,
    mut settings: ResMut<Settings>,
    mut music: Option<ResMut<Music>>,
    mut sounds: Sounds,
    mut windows: Query<&mut Window, With<PrimaryWindow>>,
) {
    let events = dialogs.take_events();
    let options_were_open = dialogs.get(DialogId::NewOptions).is_some();
    let (Some(res), Some(mut profiles)) = (res, profiles) else { return };
    let mut app = DialogActions {
        dialogs: &mut dialogs,
        res: &res,
        profiles: &mut profiles,
        changed: &mut changed,
        exit: &mut exit,
        next_state: &mut next_state,
        open_credits_note: false,
    };
    for event in events {
        match event {
            DialogEvent::Button(DialogId::NewOptions, _)
            | DialogEvent::SliderVal(DialogId::NewOptions, ..)
            | DialogEvent::CheckboxChecked(DialogId::NewOptions, ..) => {
                options_event(event, &mut app, &mut settings, music.as_deref_mut(), &mut sounds, &mut windows, board.as_deref())
            }
            DialogEvent::Button(DialogId::PurchasePacketSlot, button) => {
                let yes = button == DialogButton::Yes;
                app.finish_purchase_packet_slot(yes);
                if let Some(board) = board.as_deref_mut() {
                    board.packet_offer_answer = Some(yes);
                }
            }
            // The store's questions: its own update acts on the answer.
            DialogEvent::Button(
                id @ (DialogId::StorePurchase | DialogId::NotEnoughMoney | DialogId::Upgraded | DialogId::VisitTree),
                button,
            ) => {
                app.dialogs.kill_dialog(id);
                if let Some(store) = store.as_deref_mut() {
                    store.dialog_answer = Some((id, button == DialogButton::Yes));
                }
            }
            // The credits' pause: resume, or back to the main menu.
            DialogEvent::Button(DialogId::CreditsPause, button) => {
                app.dialogs.kill_dialog(DialogId::CreditsPause);
                if button == DialogButton::Yes {
                    if let Some(credits) = credits.as_deref_mut() {
                        credits.paused = false;
                    }
                } else {
                    app.next_state.set(AppState::GameSelector);
                }
            }
            DialogEvent::Button(DialogId::ChooserWarning, button) => {
                app.dialogs.kill_dialog(DialogId::ChooserWarning);
                if let Some(board) = board.as_deref_mut() {
                    board.answer_chooser_warning(button == DialogButton::Yes);
                }
            }
            // The Continue dialog: resume the save, restart the level (the
            // save is erased), or cancel back to the menu.
            DialogEvent::Button(DialogId::Continue, DialogButton::Continue) => {
                app.dialogs.kill_dialog(DialogId::Continue);
                commands.insert_resource(LoadSavedGame(true));
                app.next_state.set(AppState::Board);
            }
            DialogEvent::Button(DialogId::Continue, DialogButton::NewGame) => {
                app.dialogs.kill_dialog(DialogId::Continue);
                if let (Some(player), Some(dir)) = (app.profiles.player(), save_dir()) {
                    let (id, mode) = (player.id, crate::board::GameMode::Adventure);
                    erase_save(&dir, id, mode);
                }
                commands.insert_resource(LoadSavedGame(false));
                app.next_state.set(AppState::Board);
            }
            // A restart counts as a loss for the boss level's intro.
            DialogEvent::Button(DialogId::ConfirmRestart, DialogButton::Yes) => {
                commands.insert_resource(crate::board::BoardResult::Restart);
                app.button_depress(DialogId::ConfirmRestart, true, board.as_deref());
            }
            // `GameOverDialog`: Try Again (the footer) starts the level over,
            // its top-right button goes back to the main menu.
            DialogEvent::Button(DialogId::GameOver, button) => {
                app.dialogs.kill_dialog(DialogId::GameOver);
                if button == DialogButton::Yes {
                    app.next_state.set(AppState::Board);
                } else {
                    app.do_back_to_main(board.as_deref());
                }
            }
            DialogEvent::Button(id, DialogButton::Yes) => app.button_depress(id, true, board.as_deref()),
            DialogEvent::Button(id, DialogButton::No) => app.button_depress(id, false, board.as_deref()),
            DialogEvent::Button(DialogId::UserDialog, b) => app.user_dialog_button(b),
            DialogEvent::ListClicked(DialogId::UserDialog, idx, count) => app.user_list_clicked(idx, count),
            _ => {}
        }
    }
    // However the options closed (OK, Escape, leaving or restarting the
    // level), the settings are saved.
    if options_were_open && app.dialogs.get(DialogId::NewOptions).is_none() {
        settings.save();
    }
    if app.open_credits_note {
        commands.insert_resource(crate::award::AwardScreen::credits_note());
    }
}
