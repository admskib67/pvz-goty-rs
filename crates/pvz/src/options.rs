//! The player's settings: music and sound volumes, full screen and 3D
//! acceleration, set in the options dialog (`NewOptionsDialog`) and kept in
//! `config.toml`, where the original kept them in the registry.
//!
//! Behaviour per ruslan831 decomp: `LawnApp::DoNewOptions`,
//! `LawnApp::KillNewOptionsDialog`, `NewOptionsDialog::SliderVal` and
//! `SexyAppBase`'s `ReadFromRegistry`/`WriteToRegistry`. 3D acceleration is
//! remembered but changes nothing: Bevy always draws on the GPU.

use bevy::prelude::*;
use bevy::window::{MonitorSelection, PrimaryWindow, WindowMode};

use crate::audio::{MUSIC_VOLUME, Music, SFX_VOLUME, SoundSystem};
use crate::dialog::{DialogSlider, Dialogs, LawnDialog, OptionsFrom, OptionsValues, SFX_SLIDER_SCALE};
use crate::install::Config;
use crate::resources::GameResources;

pub struct OptionsPlugin;

impl Plugin for OptionsPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(Settings::from_config(&Config::load()))
            // Music is only there once Startup's commands have run.
            .add_systems(PostStartup, apply_saved_settings);
    }
}

#[derive(Resource, Debug, Clone, Copy, PartialEq)]
pub struct Settings {
    /// `mMusicVolume`, 0 to 1.
    pub music_volume: f32,
    /// `mSfxVolume`, 0 to 0.65 (the slider's full scale).
    pub sfx_volume: f32,
    /// `!mIsWindowed`.
    pub fullscreen: bool,
    /// `Is3DAccelerated()`.
    pub hardware_acceleration: bool,
}

impl Default for Settings {
    /// The original's defaults (`LawnApp::LawnApp`): windowed, accelerated.
    fn default() -> Self {
        Self { music_volume: MUSIC_VOLUME, sfx_volume: SFX_VOLUME, fullscreen: false, hardware_acceleration: true }
    }
}

impl Settings {
    /// `ReadFromRegistry`: what's saved, else the defaults.
    pub fn from_config(config: &Config) -> Self {
        let d = Self::default();
        Self {
            music_volume: config.music_volume.unwrap_or(d.music_volume).clamp(0.0, 1.0),
            sfx_volume: config.sfx_volume.unwrap_or(d.sfx_volume).clamp(0.0, SFX_SLIDER_SCALE),
            fullscreen: config.fullscreen.unwrap_or(d.fullscreen),
            hardware_acceleration: config.hardware_acceleration.unwrap_or(d.hardware_acceleration),
        }
    }

    pub fn write_to(&self, config: &mut Config) {
        config.music_volume = Some(self.music_volume);
        config.sfx_volume = Some(self.sfx_volume);
        config.fullscreen = Some(self.fullscreen);
        config.hardware_acceleration = Some(self.hardware_acceleration);
    }

    /// `WriteToRegistry`, for the settings.
    pub fn save(&self) {
        let mut config = Config::load();
        self.write_to(&mut config);
        if let Err(e) = config.save() {
            error!("couldn't save the settings: {e}");
        }
    }

    /// `NewOptionsDialog::SliderVal`: a slider's value becomes a volume.
    pub fn set_slider(&mut self, slider: DialogSlider, value: f32) {
        match slider {
            DialogSlider::Music => self.music_volume = value.clamp(0.0, 1.0),
            DialogSlider::Sfx => self.sfx_volume = value.clamp(0.0, 1.0) * SFX_SLIDER_SCALE,
        }
    }

    /// The volumes, for the music and sounds to use.
    pub fn apply_volumes(&self, music: &mut Music, sounds: &mut SoundSystem) {
        music.set_volume(self.music_volume);
        sounds.sfx_volume = self.sfx_volume;
    }

    pub fn window_mode(&self) -> WindowMode {
        if self.fullscreen { WindowMode::BorderlessFullscreen(MonitorSelection::Current) } else { WindowMode::Windowed }
    }
}

fn apply_saved_settings(
    settings: Res<Settings>,
    mut music: ResMut<Music>,
    mut sounds: ResMut<SoundSystem>,
    mut windows: Query<&mut Window, With<PrimaryWindow>>,
) {
    settings.apply_volumes(&mut music, &mut sounds);
    for mut window in &mut windows {
        window.mode = settings.window_mode();
    }
}

/// `LawnApp::DoNewOptions`.
pub fn do_new_options(dialogs: &mut Dialogs, res: &GameResources, settings: &Settings, from: OptionsFrom) {
    let values = OptionsValues {
        music_volume: settings.music_volume,
        sfx_volume: settings.sfx_volume,
        fullscreen: settings.fullscreen,
        hardware_acceleration: settings.hardware_acceleration,
    };
    dialogs.add_dialog(LawnDialog::new_options(res, values, from));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_settings_are_the_originals_defaults() {
        let config: Config = toml::from_str("cur_user = \"Ann\"\n").unwrap();
        assert_eq!(Settings::from_config(&config), Settings::default());
        assert_eq!(Settings::default().music_volume, 0.85);
        assert_eq!(Settings::default().sfx_volume, 0.5525);
    }

    #[test]
    fn settings_survive_a_round_trip_through_the_config() {
        let settings = Settings { music_volume: 0.25, sfx_volume: 0.5, fullscreen: true, hardware_acceleration: false };
        let mut config = Config::default();
        settings.write_to(&mut config);
        let text = toml::to_string_pretty(&config).unwrap();
        let back: Config = toml::from_str(&text).unwrap();
        assert_eq!(Settings::from_config(&back), settings);
    }

    #[test]
    fn sound_slider_is_scaled_and_saved_values_clamped() {
        let mut settings = Settings::default();
        settings.set_slider(DialogSlider::Sfx, 1.0);
        assert_eq!(settings.sfx_volume, 0.65);
        settings.set_slider(DialogSlider::Music, 0.4);
        assert_eq!(settings.music_volume, 0.4);
        let config = Config { music_volume: Some(3.0), sfx_volume: Some(-1.0), ..Config::default() };
        let loaded = Settings::from_config(&config);
        assert_eq!((loaded.music_volume, loaded.sfx_volume), (1.0, 0.0));
    }
}
