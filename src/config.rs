use std::{env, fs, io, path::PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Clone, PartialEq, Eq, Serialize)]
pub struct JiraCredentials {
    pub site: String,
    pub email: String,
    pub api_key: String,
    pub default_project: String,
}

impl JiraCredentials {
    pub fn is_complete(&self) -> bool {
        !self.site.trim().is_empty()
            && !self.email.trim().is_empty()
            && !self.api_key.trim().is_empty()
            && !self.default_project.trim().is_empty()
    }
}

#[derive(Debug, Deserialize)]
struct ConfigFile {
    atlassian: Option<AtlassianConfig>,
}

#[derive(Debug, Deserialize)]
struct AtlassianConfig {
    site: Option<String>,
    email: Option<String>,
    api_key: Option<String>,
    default_project: Option<String>,
}

#[derive(Serialize)]
struct WritableConfig<'a> {
    atlassian: &'a JiraCredentials,
}

pub fn load_jira_credentials() -> Option<JiraCredentials> {
    let path = config_path()?;
    load_jira_credentials_from_path(path).ok().flatten()
}

pub fn save_jira_credentials(credentials: &JiraCredentials) -> io::Result<()> {
    let path = config_path().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            "HOME is not set for ~/.tira/config.toml",
        )
    })?;
    save_jira_credentials_to_path(path, credentials)
}

pub fn load_jira_credentials_from_path(
    path: impl Into<PathBuf>,
) -> io::Result<Option<JiraCredentials>> {
    let text = fs::read_to_string(path.into())?;
    Ok(jira_credentials_from_toml(&text))
}

pub fn jira_credentials_from_toml(text: &str) -> Option<JiraCredentials> {
    let config = toml::from_str::<ConfigFile>(text).ok()?;
    let atlassian = config.atlassian?;
    let credentials = JiraCredentials {
        site: atlassian.site?,
        email: atlassian.email?,
        api_key: atlassian.api_key?,
        default_project: atlassian.default_project?,
    };

    credentials.is_complete().then_some(credentials)
}

pub fn save_jira_credentials_to_path(
    path: impl Into<PathBuf>,
    credentials: &JiraCredentials,
) -> io::Result<()> {
    let path = path.into();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
        secure_dir(parent)?;
    }

    let contents = toml::to_string(&WritableConfig {
        atlassian: credentials,
    })
    .map_err(io::Error::other)?;

    fs::write(&path, contents)?;
    secure_file(&path)?;
    Ok(())
}

fn config_path() -> Option<PathBuf> {
    let home = env::var_os("HOME")?;
    Some(PathBuf::from(home).join(".tira/config.toml"))
}

#[cfg(unix)]
fn secure_dir(path: &std::path::Path) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt;

    fs::set_permissions(path, fs::Permissions::from_mode(0o700))
}

#[cfg(not(unix))]
fn secure_dir(_path: &std::path::Path) -> io::Result<()> {
    Ok(())
}

#[cfg(unix)]
fn secure_file(path: &std::path::Path) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt;

    fs::set_permissions(path, fs::Permissions::from_mode(0o600))
}

#[cfg(not(unix))]
fn secure_file(_path: &std::path::Path) -> io::Result<()> {
    Ok(())
}
