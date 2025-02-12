use std::path::{Path, PathBuf};

use rand::random;

use cli_state::error::Result;
use ockam::SqlxDatabase;
use ockam_core::env::get_env_with_default;
use ockam_node::Executor;

use crate::cli_state;
use crate::cli_state::CliStateError;

/// The CliState struct manages all the data persisted locally.
///
/// The data is mostly saved to one database file (there can be additional files if distinct vaults are created)
/// accessed with the SqlxDatabase struct.
///
/// However all the SQL queries for creating / updating / deleting entities are implemented by repositories,
/// for each data type: Project, Space, Vault, Identity, etc...
///
/// The repositories themselves are not accessible from the `CliState` directly since it is often
/// necessary to use more than one repository to implement a given behaviour. For example deleting
/// an identity requires to query the nodes that are using that identity and only delete it if no
/// node is using that identity
///
#[derive(Debug, Clone)]
pub struct CliState {
    dir: PathBuf,
    database: SqlxDatabase,
}

impl CliState {
    /// Create a new CliState in a given directory
    pub fn new(dir: &Path) -> Result<Self> {
        Executor::execute_future(Self::create(dir.into()))?
    }

    pub fn dir(&self) -> PathBuf {
        self.dir.clone()
    }

    pub fn database(&self) -> SqlxDatabase {
        self.database.clone()
    }

    pub fn database_path(&self) -> PathBuf {
        Self::make_database_path(&self.dir)
    }

    pub fn set_node_name(&mut self, node_name: String) {
        self.database.node_name = Some(node_name)
    }
}

/// These functions allow to create and reset the local state
impl CliState {
    /// Return a new CliState using a default directory to store its data
    pub fn with_default_dir() -> Result<Self> {
        Self::new(Self::default_dir()?.as_path())
    }

    /// Stop nodes and remove all the directories storing state
    pub async fn reset(&self) -> Result<()> {
        self.delete_all_named_identities().await?;
        self.delete_all_nodes(true).await?;
        self.delete_all_named_vaults().await?;
        self.delete()
    }

    /// Delete the local database and log files
    pub fn delete(&self) -> Result<()> {
        Self::delete_at(&self.dir)
    }

    /// Reset all directories and return a new CliState
    pub async fn recreate(&self) -> Result<CliState> {
        self.reset().await?;
        Self::create(self.dir.clone()).await
    }

    /// Backup and reset is used to save aside
    /// some corrupted local state for later inspection and then reset the state
    pub fn backup_and_reset() -> Result<()> {
        let dir = Self::default_dir()?;

        // Reset backup directory
        let backup_dir = Self::backup_default_dir()?;
        if backup_dir.exists() {
            let _ = std::fs::remove_dir_all(&backup_dir);
        }
        std::fs::create_dir_all(&backup_dir)?;

        // Move state to backup directory
        for entry in std::fs::read_dir(&dir)? {
            let entry = entry?;
            let from = entry.path();
            let to = backup_dir.join(entry.file_name());
            std::fs::rename(from, to)?;
        }

        // Reset state
        Self::delete_at(&dir)?;
        let state = Self::new(&dir)?;

        let dir = &state.dir;
        let backup_dir = CliState::backup_default_dir().unwrap();
        eprintln!("The {dir:?} directory has been reset and has been backed up to {backup_dir:?}");
        Ok(())
    }

    /// Returns the default backup directory for the CLI state.
    pub fn backup_default_dir() -> Result<PathBuf> {
        let dir = Self::default_dir()?;
        let dir_name =
            dir.file_name()
                .and_then(|n| n.to_str())
                .ok_or(CliStateError::InvalidOperation(
                    "The $OCKAM_HOME directory does not have a valid name".to_string(),
                ))?;
        let parent = dir.parent().ok_or(CliStateError::InvalidOperation(
            "The $OCKAM_HOME directory does not a valid parent directory".to_string(),
        ))?;
        Ok(parent.join(format!("{dir_name}.bak")))
    }
}

/// Low-level functions for creating / deleting CliState files
impl CliState {
    /// Create a new CliState where the data is stored at a given path
    pub(super) async fn create(dir: PathBuf) -> Result<Self> {
        std::fs::create_dir_all(&dir)?;
        let database = SqlxDatabase::create(Self::make_database_path(&dir)).await?;
        debug!("Opened the database with options {:?}", database);
        let state = Self { dir, database };
        Ok(state)
    }

    pub(super) fn make_database_path(root_path: &Path) -> PathBuf {
        root_path.join("database.sqlite3")
    }

    pub(super) fn make_node_dir_path(root_path: &Path, node_name: &str) -> PathBuf {
        Self::make_nodes_dir_path(root_path).join(node_name)
    }

    pub(super) fn make_nodes_dir_path(root_path: &Path) -> PathBuf {
        root_path.join("nodes")
    }

    /// Delete the state files
    fn delete_at(root_path: &Path) -> Result<()> {
        // Delete nodes logs
        let _ = std::fs::remove_dir_all(Self::make_nodes_dir_path(root_path));
        // Delete the database
        let _ = std::fs::remove_file(Self::make_database_path(root_path));
        // If the state directory is now empty, delete it
        let _ = std::fs::remove_dir(root_path);
        Ok(())
    }

    /// Returns the default directory for the CLI state.
    /// That directory is determined by `OCKAM_HOME` environment variable and is
    /// $OCKAM_HOME/.ockam.
    ///
    /// If $OCKAM_HOME is not defined then $HOME is used instead
    fn default_dir() -> Result<PathBuf> {
        Ok(get_env_with_default::<PathBuf>(
            "OCKAM_HOME",
            home::home_dir()
                .ok_or(CliStateError::InvalidPath("$HOME".to_string()))?
                .join(".ockam"),
        )?)
    }
}

/// Return a random, but memorable, name which can be used to name identities, nodes, vaults, etc...
pub fn random_name() -> String {
    petname::petname(2, "-").unwrap_or(hex::encode(random::<[u8; 4]>()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use itertools::Itertools;
    use std::fs;
    use tempfile::NamedTempFile;

    #[tokio::test]
    async fn test_reset() -> Result<()> {
        let db_file = NamedTempFile::new().unwrap();
        let cli_state_directory = db_file.path().parent().unwrap().join(random_name());
        let cli = CliState::create(cli_state_directory.clone()).await?;

        // create 2 vaults
        // the second vault is using a separate file
        let _vault1 = cli.get_or_create_named_vault("vault1").await?;
        let _vault2 = cli.get_or_create_named_vault("vault2").await?;

        // create 2 identities
        let identity1 = cli
            .create_identity_with_name_and_vault("identity1", "vault1")
            .await?;
        let identity2 = cli
            .create_identity_with_name_and_vault("identity2", "vault2")
            .await?;

        // create 2 nodes
        let _node1 = cli
            .create_node_with_identifier("node1", &identity1.identifier())
            .await?;
        let _node2 = cli
            .create_node_with_identifier("node2", &identity2.identifier())
            .await?;

        let file_names = list_file_names(&cli_state_directory);
        assert_eq!(
            file_names.iter().sorted().as_slice(),
            ["vault-vault2".to_string(), "database.sqlite3".to_string()]
                .iter()
                .sorted()
                .as_slice()
        );

        // reset the local state
        cli.reset().await?;
        let result = fs::read_dir(cli_state_directory);
        assert!(result.is_err(), "the cli state directory is deleted");

        Ok(())
    }

    /// HELPERS
    fn list_file_names(dir: &Path) -> Vec<String> {
        fs::read_dir(dir)
            .unwrap()
            .map(|f| f.unwrap().file_name().to_string_lossy().to_string())
            .collect()
    }
}
