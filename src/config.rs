use color_eyre::eyre::eyre;
use color_eyre::Result;
use notify::EventKind;
use notify::RecommendedWatcher;
use notify::RecursiveMode;
use notify::Watcher;
use parking_lot::RwLock;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

const DEFAULT_CONFIG: &str = include_str!("../masir.json.default");

macro_rules! define_config {
    (
        $(
            $(#[$field_meta:meta])*
            $field_name:ident : $field_type:ty = $default:expr
        ),* $(,)?
    ) => {

        #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
        struct ConfigData {
            $(
                #[serde(default)]
                pub $field_name: $field_type,
            )*
        }

        impl Default for ConfigData {
            fn default() -> Self {
                serde_json::from_str(DEFAULT_CONFIG).unwrap_or(Self {
                    $($field_name: $default,)*
                })
            }
        }

        #[derive(Debug, Clone, Default)]
        pub struct ConfigOverrides {
            $(pub $field_name: Option<$field_type>,)*
        }

        #[allow(dead_code)]
        impl Config {
            $(
                $(#[$field_meta])*
                pub fn $field_name(&self) -> $field_type {
                    self.overrides
                        .$field_name
                        .unwrap_or_else(|| self.data.read().$field_name.clone())
                }

                paste::paste! {
                    pub fn [<set_ $field_name>](&self, value: $field_type) -> Result<()> {
                        self.data.write().$field_name = value;
                        self.save()
                    }
                }
            )*
        }
    };
}

define_config! {
    /// Whether to focus fullscreen windows when the cursor moves over them.
    /// When `true`, windows that are fullscreen or have dimensions equal to
    /// their monitor will be focused on mouse hover.
    focus_fullscreen_windows: bool = true,
}

pub struct Config {
    data: Arc<RwLock<ConfigData>>,
    overrides: ConfigOverrides,
    #[allow(dead_code)]
    watcher: Option<RecommendedWatcher>,
}

impl std::fmt::Debug for Config {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Config")
            .field("data", &*self.data.read())
            .field("overrides", &self.overrides)
            .finish()
    }
}

impl Config {
    /**
     * Defaults to `$HOME/.config/masir.json` or `C:\Users\<User>\.config\masir.json`
     */
    pub fn path() -> Result<PathBuf> {
        let home = dirs::home_dir().ok_or_else(|| eyre!("could not determine home directory"))?;

        Ok(home.join(".config").join("masir.json"))
    }

    pub fn load_or_create(overrides: ConfigOverrides) -> Result<Arc<Self>> {
        let config_path = Self::path()?;
        let data = Self::load_data(&config_path)?;
        let data = Arc::new(RwLock::new(data));

        let watcher = Self::setup_watcher(Arc::clone(&data), &config_path)?;

        Ok(Arc::new(Self {
            data,
            overrides,
            watcher: Some(watcher),
        }))
    }

    fn load_data(config_path: &PathBuf) -> Result<ConfigData> {
        if config_path.exists() {
            let contents = fs::read_to_string(config_path)?;
            let mut data: ConfigData = serde_json::from_str(&contents)?;

            if Self::migrate_if_needed(config_path, &contents)? {
                let updated_contents = fs::read_to_string(config_path)?;
                data = serde_json::from_str(&updated_contents)?;
            }

            tracing::info!("loaded config from {}", config_path.display());
            Ok(data)
        } else {
            let data = ConfigData::default();
            Self::save_data(config_path, &data)?;
            tracing::info!("created default config at {}", config_path.display());
            Ok(data)
        }
    }

    fn setup_watcher(
        data: Arc<RwLock<ConfigData>>,
        config_path: &Path,
    ) -> Result<RecommendedWatcher> {
        let path_for_watcher = config_path.to_path_buf();

        let mut watcher =
            notify::recommended_watcher(move |res: notify::Result<notify::Event>| match res {
                Ok(event) => {
                    if matches!(event.kind, EventKind::Modify(_) | EventKind::Create(_)) {
                        Self::handle_file_change(&data, &path_for_watcher);
                    }
                }
                Err(e) => {
                    tracing::error!("config file watcher error: {}", e);
                }
            })?;

        if let Some(parent) = config_path.parent() {
            watcher.watch(parent, RecursiveMode::NonRecursive)?;
            tracing::info!("watching for config changes in {}", parent.display());
        }

        Ok(watcher)
    }

    fn handle_file_change(data: &Arc<RwLock<ConfigData>>, config_path: &Path) {
        match fs::read_to_string(config_path)
            .map_err(|e| e.into())
            .and_then(|contents| {
                serde_json::from_str::<ConfigData>(&contents).map_err(|e| eyre!(e))
            }) {
            Ok(new_data) => {
                let mut current = data.write();
                if *current != new_data {
                    tracing::info!("config file changed, reloading");
                    tracing::debug!("new config: {:?}", new_data);
                    *current = new_data;
                }
            }
            Err(e) => {
                tracing::warn!("config file changed but failed to parse: {}", e);
                tracing::warn!("keeping previous config");
            }
        }
    }

    /// If there are fields in the default config file that aren't in the current config file,
    /// add them with their default values.
    fn migrate_if_needed(config_path: &PathBuf, current_contents: &str) -> Result<bool> {
        let current: Value = serde_json::from_str(current_contents)?;
        let default: Value = serde_json::from_str(DEFAULT_CONFIG)?;

        let Some(current_obj) = current.as_object() else {
            return Ok(false);
        };

        let Some(default_obj) = default.as_object() else {
            return Ok(false);
        };

        let mut merged = current_obj.clone();
        let mut migrated = false;

        for (key, value) in default_obj {
            if !current_obj.contains_key(key) {
                tracing::info!("migrating config: adding new field '{}'", key);
                merged.insert(key.clone(), value.clone());
                migrated = true;
            }
        }

        if migrated {
            let merged_json = serde_json::to_string_pretty(&merged)?;
            fs::write(config_path, merged_json)?;
            tracing::info!("config migration complete");
        }

        Ok(migrated)
    }

    fn save_data(config_path: &PathBuf, data: &ConfigData) -> Result<()> {
        if let Some(parent) = config_path.parent() {
            fs::create_dir_all(parent)?;
        }

        let contents = serde_json::to_string_pretty(data)?;
        fs::write(config_path, contents)?;

        Ok(())
    }

    pub fn save(&self) -> Result<()> {
        let config_path = Self::path()?;
        Self::save_data(&config_path, &self.data.read())
    }
}
