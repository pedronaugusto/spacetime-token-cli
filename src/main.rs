use anyhow::{Context, Result};
use clap::Parser;
use dialoguer::{theme::ColorfulTheme, Select};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, HashMap},
    fs,
    path::PathBuf,
    process::Command as StdCommand,
};
use toml_edit::{DocumentMut, Item};
use reqwest::blocking::Client as BlockingHttpClient;
use reqwest::header::CONTENT_LENGTH;

const APP_DIR_NAME: &str = "spacetime-token"; // Renamed
const DEFAULT_PROFILES_FILENAME: &str = "profiles.toml"; // Renamed
const DEFAULT_CONFIG_FILENAME: &str = "config.toml";
const SPACETIME_CLI_COMMAND: &str = "spacetime";

#[derive(Debug, Deserialize, Serialize)]
struct AppSettings {
    profiles_filename: String, // Renamed
    cli_config_dir_from_home: String,
    cli_config_filename: String,
    cli_token_key: String,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            profiles_filename: DEFAULT_PROFILES_FILENAME.to_string(), // Renamed
            cli_config_dir_from_home: ".config/spacetime".to_string(),
            cli_config_filename: "cli.toml".to_string(),
            cli_token_key: "spacetimedb_token".to_string(),
        }
    }
}

#[derive(Parser, Debug)]
#[clap(
    name = "spacetime-token", // Renamed
    version = "0.1.0",
    about = "Manages SpacetimeDB tokens via profiles" // Updated about
)]
struct Cli {
    #[clap(subcommand)]
    command: Commands,
}

#[derive(Parser, Debug)]
enum Commands {
    /// Saves/updates a profile with a token and sets it active
    Set(SetArgs),
    /// Saves the current active token from cli.toml to a new profile name
    Save(SaveArgs),
    /// Resets (clears) the profiles.toml file
    Reset(ResetArgs),
    /// Creates a new profile via 'spacetime login' and saves the token
    Create(CreateArgs),
    /// Lists all stored profile names
    List(ListArgs),
    /// Deletes a stored profile
    Delete(DeleteArgs),
    /// Interactive setup for configuration values
    Setup,
    /// Switches the active token to a stored profile
    Switch(SwitchArgs),
    /// Displays the current active profile name and token (masked)
    Current,
    /// Switches to the admin profile
    Admin,
    /// Manage or inspect environments (server addresses)
    Env(EnvArgs),
    /// Updates the address of an existing profile
    SetAddress(SetAddressArgs),
}

#[derive(Parser, Debug)]
struct SetArgs {
    /// The profile name to save/update
    profile_name: String,
    /// The token to associate with the profile name
    token: String,
    /// The server address (e.g., 'local' or 'http://remote.host/spacetime')
    #[clap(long)]
    address: Option<String>,
}

#[derive(Parser, Debug)]
struct SwitchArgs {
    /// The profile name of the stored profile to make active (optional)
    profile_name: Option<String>, // Renamed
    /// Override the environment filter with a specific address
    #[clap(long)]
    address: Option<String>,
}

#[derive(Parser, Debug)]
struct SaveArgs {
    /// The profile name to save the current active token under
    profile_name: String, // Renamed
}

#[derive(Parser, Debug)]
struct CreateArgs {
    /// The profile name for the new profile
    profile_name: String,
    /// The server address (e.g., 'local' or 'http://remote.host/spacetime')
    #[clap(long)]
    address: Option<String>,
}

#[derive(Parser, Debug)]
struct ListArgs {
    /// Only show profiles for the current environment
    #[clap(long)]
    env: bool,
}

#[derive(Parser, Debug)]
struct DeleteArgs {
    /// The profile name of the profile to delete
    profile_name: String,
    /// Forces deletion without confirmation
    #[clap(long, short)]
    force: bool,
}

#[derive(Parser, Debug)]
struct ResetArgs {
    /// Forces reset without confirmation
    #[clap(long, short)]
    force: bool,
}

#[derive(Parser, Debug)]
struct SetAddressArgs {
    /// The profile name to update
    profile_name: String,
    /// The new server address
    address: String,
}

#[derive(Parser, Debug)]
struct EnvArgs {
    #[clap(subcommand)]
    command: Option<EnvCommands>,
}

#[derive(Parser, Debug)]
enum EnvCommands {
    /// Show the current environment from the CLI config
    Current,
    /// List known environments from saved profiles
    List,
    /// Set the active environment and optionally switch to a matching profile
    Use(EnvUseArgs),
}

#[derive(Parser, Debug)]
struct EnvUseArgs {
    /// The address to set as the current environment
    address: String,
    /// The profile to activate while setting the environment
    #[clap(long, short)]
    profile: Option<String>,
}

#[derive(Serialize, Deserialize, Debug)]
struct IdentityResponse {
    token: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct Profile {
    token: String,
    address: String,
}

#[derive(Serialize, Deserialize, Debug, Default)]
struct UserProfiles(HashMap<String, Profile>);

fn get_app_config_dir() -> Result<PathBuf> {
    let config_dir = dirs::config_dir()
        .context("Failed to get user's config directory.")?
        .join(APP_DIR_NAME);
    if !config_dir.exists() {
        fs::create_dir_all(&config_dir).with_context(|| {
            format!("Failed to create app config directory at {:?}", config_dir)
        })?;
        println!("Created application config directory at {:?}", config_dir);
    }
    Ok(config_dir)
}

fn load_app_settings() -> Result<AppSettings> {
    let app_config_dir = get_app_config_dir()?;
    let config_file_path = app_config_dir.join(DEFAULT_CONFIG_FILENAME);

    if !config_file_path.exists() {
        println!(
            "Configuration file not found at {:?}. Creating with default settings.",
            config_file_path
        );
        let default_settings = AppSettings::default();
        let toml_content = toml::to_string_pretty(&default_settings)
            .context("Failed to serialize default settings to TOML")?;
        fs::write(&config_file_path, toml_content)
            .with_context(|| format!("Failed to write default config to {:?}", config_file_path))?;
        return Ok(default_settings);
    }

    let content = fs::read_to_string(&config_file_path)
        .with_context(|| format!("Failed to read app config file at {:?}", config_file_path))?;
    toml::from_str(&content)
        .with_context(|| format!("Failed to parse app config file at {:?}", config_file_path))
}

fn write_app_settings(settings: &AppSettings) -> Result<()> {
    let app_config_dir = get_app_config_dir()?;
    let config_file_path = app_config_dir.join(DEFAULT_CONFIG_FILENAME);
    let toml_content =
        toml::to_string_pretty(settings).context("Failed to serialize app settings to TOML")?;
    fs::write(&config_file_path, toml_content)
        .with_context(|| format!("Failed to write app config to {:?}", config_file_path))?;
    println!("Configuration saved to {:?}", config_file_path);
    Ok(())
}

fn get_profiles_filepath(settings: &AppSettings) -> Result<PathBuf> {
    // Renamed function
    let app_config_dir = get_app_config_dir()?;
    Ok(app_config_dir.join(&settings.profiles_filename)) // Renamed field
}

fn get_cli_toml_path(settings: &AppSettings) -> Result<PathBuf> {
    let home_dir = dirs::home_dir().context("Failed to get home directory")?;
    Ok(home_dir
        .join(&settings.cli_config_dir_from_home)
        .join(&settings.cli_config_filename))
}

fn read_profiles(settings: &AppSettings) -> Result<UserProfiles> {
    let profiles_path = get_profiles_filepath(settings)?;
    if !profiles_path.exists() {
        fs::write(&profiles_path, "").with_context(|| {
            format!(
                "Failed to create empty profiles file at {:?}",
                profiles_path
            )
        })?;
        println!("Created empty {}.", settings.profiles_filename);
        return Ok(UserProfiles::default());
    }

    let content = fs::read_to_string(&profiles_path)
        .with_context(|| format!("Failed to read profiles file at {:?}", profiles_path))?;
    if content.trim().is_empty() {
        return Ok(UserProfiles::default());
    }

    // Try parsing new format first
    match toml::from_str::<UserProfiles>(&content) {
        Ok(profiles) => Ok(profiles),
        Err(e) => {
            // If it fails, try parsing the old format and migrating
            println!(
                "Could not parse profiles file. Assuming old format and attempting migration..."
            );

            #[derive(Deserialize)]
            struct OldUserProfiles(HashMap<String, String>);

            match toml::from_str::<OldUserProfiles>(&content) {
                Ok(old_profiles) => {
                    let mut new_profiles = UserProfiles::default();
                    for (name, token) in old_profiles.0 {
                        new_profiles.0.insert(
                            name,
                            Profile {
                                token,
                                address: "local".to_string(),
                            },
                        );
                    }
                    // Write the migrated profiles back to the file
                    write_profiles(settings, &new_profiles)
                        .context("Failed to save migrated profiles file.")?;
                    println!("Successfully migrated profiles to new format.");
                    Ok(new_profiles)
                }
                Err(migration_err) => {
                    println!(
                        "Failed to parse profiles file as old format either: {}",
                        migration_err
                    );
                    Err(anyhow::Error::new(e).context(format!(
                        "Failed to parse profiles file at {:?}. It might be corrupted.",
                        profiles_path
                    )))
                }
            }
        }
    }
}

fn write_profiles(settings: &AppSettings, profiles: &UserProfiles) -> Result<()> {
    // Renamed function and param
    let profiles_path = get_profiles_filepath(settings)?; // Renamed variable
    let content =
        toml::to_string_pretty(profiles).context("Failed to serialize profiles data to TOML")?; // Renamed
    fs::write(&profiles_path, content) // Renamed variable
        .with_context(|| format!("Failed to write profiles file at {:?}", profiles_path))?; // Renamed
    println!("Successfully updated {}.", settings.profiles_filename); // Renamed field
    Ok(())
}

fn read_cli_toml(settings: &AppSettings) -> Result<DocumentMut> {
    let path = get_cli_toml_path(settings)?;
    let content = fs::read_to_string(&path).with_context(|| {
        format!(
            "Failed to read {} from {:?}",
            settings.cli_config_filename, path
        )
    })?;
    content.parse::<DocumentMut>().with_context(|| {
        format!(
            "Failed to parse {} from {:?}",
            settings.cli_config_filename, path
        )
    })
}

fn write_cli_toml(settings: &AppSettings, doc: &DocumentMut) -> Result<()> {
    let path = get_cli_toml_path(settings)?;
    fs::write(&path, doc.to_string()).with_context(|| {
        format!(
            "Failed to write {} to {:?}",
            settings.cli_config_filename, path
        )
    })?;
    println!("Successfully updated {}.", settings.cli_config_filename);
    Ok(())
}

fn load_or_init_cli_toml(settings: &AppSettings) -> Result<DocumentMut> {
    let path = get_cli_toml_path(settings)?;
    if let Some(parent_dir) = path.parent() {
        fs::create_dir_all(parent_dir)
            .with_context(|| format!("Failed to create directory {:?}", parent_dir))?;
    }

    if path.exists() {
        read_cli_toml(settings)
    } else {
        Ok(DocumentMut::new())
    }
}

fn get_current_environment(settings: &AppSettings) -> Result<Option<String>> {
    let cli_toml_path = get_cli_toml_path(settings)?;
    if !cli_toml_path.exists() {
        return Ok(None);
    }
    let cli_toml = read_cli_toml(settings)?;
    Ok(cli_toml
        .get("default_host")
        .and_then(|item| item.as_str())
        .map(|s| s.to_string()))
}

fn run_external_command(command_name: &str, args: &[&str]) -> Result<()> {
    println!("Running: {} {}...", command_name, args.join(" "));
    let mut cmd = StdCommand::new(command_name);
    cmd.args(args);

    let status = cmd
        .stdin(std::process::Stdio::inherit())
        .stdout(std::process::Stdio::inherit())
        .stderr(std::process::Stdio::inherit())
        .status()
        .with_context(|| {
            format!(
                "Failed to execute command: {}. Is '{}' in your PATH?",
                command_name, command_name
            )
        })?;

    if status.success() {
        println!(
            "Command '{} {}' executed successfully.",
            command_name,
            args.join(" ")
        );
        Ok(())
    } else {
        anyhow::bail!(
            "Command '{} {}' failed with status: {}",
            command_name,
            args.join(" "),
            status
        );
    }
}

fn mask_token(token: &str) -> String {
    if token.len() <= 10 {
        // Arbitrary length, too short to mask meaningfully
        return token.to_string();
    }
    format!("{}...{}", &token[..5], &token[token.len() - 5..])
}

fn normalize_identity_base(address: &str) -> String {
    let trimmed = address.trim_end_matches('/');
    trimmed
        .strip_suffix("/spacetime")
        .unwrap_or(trimmed)
        .to_string()
}

fn normalize_server_target(address: &str) -> (String, String) {
    if address == "local" {
        return ("http".to_string(), "127.0.0.1:3000".to_string());
    }
    let trimmed = address.trim_end_matches('/');
    let trimmed = trimmed
        .strip_suffix("/spacetime")
        .unwrap_or(trimmed)
        .trim_end_matches('/');
    if let Some(host) = trimmed.strip_prefix("https://") {
        return ("https".to_string(), host.split('/').next().unwrap_or("").to_string());
    }
    if let Some(host) = trimmed.strip_prefix("http://") {
        return ("http".to_string(), host.split('/').next().unwrap_or("").to_string());
    }
    ("http".to_string(), trimmed.split('/').next().unwrap_or("").to_string())
}

fn update_cli_server_target(cli_toml: &mut DocumentMut, profile_name: &str, address: &str) {
    let (protocol, host) = normalize_server_target(address);
    cli_toml["default_server"] = Item::Value(profile_name.into());

    if cli_toml.get("server_configs").is_none() {
        cli_toml["server_configs"] = Item::ArrayOfTables(Default::default());
    }
    if let Some(array) = cli_toml["server_configs"].as_array_of_tables_mut() {
        for table in array.iter_mut() {
            if table.get("nickname").and_then(|v| v.as_str()) == Some(profile_name) {
                table["host"] = Item::Value(host.clone().into());
                table["protocol"] = Item::Value(protocol.clone().into());
                return;
            }
        }
        let mut table = toml_edit::Table::new();
        table["nickname"] = Item::Value(profile_name.into());
        table["host"] = Item::Value(host.into());
        table["protocol"] = Item::Value(protocol.into());
        array.push(table);
    }
}

fn sync_server_configs_from_profiles(cli_toml: &mut DocumentMut, profiles: &UserProfiles) {
    if cli_toml.get("server_configs").is_none() {
        cli_toml["server_configs"] = Item::ArrayOfTables(Default::default());
    }
    if let Some(array) = cli_toml["server_configs"].as_array_of_tables_mut() {
        for (name, profile) in profiles.0.iter() {
            let (protocol, host) = normalize_server_target(&profile.address);
            let mut updated = false;
            for table in array.iter_mut() {
                if table.get("nickname").and_then(|v| v.as_str()) == Some(name.as_str()) {
                    table["host"] = Item::Value(host.clone().into());
                    table["protocol"] = Item::Value(protocol.clone().into());
                    updated = true;
                    break;
                }
            }
            if !updated {
                let mut table = toml_edit::Table::new();
                table["nickname"] = Item::Value(name.clone().into());
                table["host"] = Item::Value(host.into());
                table["protocol"] = Item::Value(protocol.into());
                array.push(table);
            }
        }
    }
}

fn fetch_server_issued_token(address: &str) -> Result<String> {
    let base = normalize_identity_base(address);
    let url = format!("{}/v1/identity", base);
    let client = BlockingHttpClient::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .context("Failed to build HTTP client")?;
    let response = client
        .post(&url)
        .header(CONTENT_LENGTH, "0")
        .send()
        .with_context(|| format!("Failed to call {}", url))?;
    if !response.status().is_success() {
        anyhow::bail!(
            "Server-issued login failed with status {} for {}",
            response.status(),
            url
        );
    }
    let identity = response
        .json::<IdentityResponse>()
        .context("Failed to parse identity response")?;
    if identity.token.trim().is_empty() {
        anyhow::bail!("Identity response did not include a token.");
    }
    Ok(identity.token)
}

fn main() -> Result<()> {
    let settings = load_app_settings().context("Failed to load application settings")?;
    let cli = Cli::parse();

    match cli.command {
        Commands::Set(args) => {
            let mut profiles = read_profiles(&settings)?;
            let address = args.address.unwrap_or_else(|| {
                get_current_environment(&settings)
                    .unwrap_or_default()
                    .unwrap_or_else(|| "local".to_string())
            });
            let profile = Profile {
                token: args.token.clone(),
                address,
            };
            profiles
                .0
                .insert(args.profile_name.clone(), profile.clone());
            write_profiles(&settings, &profiles)?;
            println!(
                "Profile '{}' saved/updated in {}.",
                args.profile_name, settings.profiles_filename
            );

            let mut cli_toml = load_or_init_cli_toml(&settings)?;
            cli_toml[&settings.cli_token_key] = Item::Value(args.token.into());
            cli_toml["default_host"] = Item::Value(profile.address.into());
            update_cli_server_target(
                &mut cli_toml,
                &args.profile_name,
                &profiles.0[&args.profile_name].address,
            );
            sync_server_configs_from_profiles(&mut cli_toml, &profiles);
            write_cli_toml(&settings, &cli_toml)?;
            println!(
                "Profile '{}' also set as active in {}.",
                args.profile_name, settings.cli_config_filename
            );
        }
        Commands::Switch(args) => {
            let profiles = read_profiles(&settings)?;
            // Only filter when an address is explicitly provided; otherwise show all profiles
            let env_filter = args.address.clone();

            let profile_name_to_switch = match args.profile_name {
                Some(name) => {
                    if let Some(filter) = &env_filter {
                        if let Some(profile) = profiles.0.get(&name) {
                            if &profile.address != filter {
                                anyhow::bail!(
                                    "Profile '{}' uses address '{}' which does not match the requested environment '{}'.",
                                    name,
                                    profile.address,
                                    filter
                                );
                            }
                        }
                    }
                    name
                }
                None => {
                    let mut filtered_profiles: HashMap<String, Profile> = profiles.0.clone();
                    if let Some(env) = &env_filter {
                        println!("Environment filter: {}", env);
                        filtered_profiles.retain(|_, profile| &profile.address == env);
                    }

                    if filtered_profiles.is_empty() {
                        println!(
                            "No profiles found in {}{}.",
                            settings.profiles_filename,
                            env_filter
                                .as_ref()
                                .map(|env| format!(" for environment '{}'", env))
                                .unwrap_or_default()
                        );
                        anyhow::bail!("No profiles available to switch.");
                    }
                    let mut profile_names: Vec<String> =
                        filtered_profiles.keys().cloned().collect();
                    profile_names.sort();
                    let selection = Select::with_theme(&ColorfulTheme::default())
                        .with_prompt("Select a profile to switch to")
                        .items(&profile_names)
                        .default(0)
                        .interact_opt()?
                        .context("No profile selected or selection cancelled.")?;

                    profile_names[selection].clone()
                }
            };

            if let Some(profile_to_switch) = profiles.0.get(&profile_name_to_switch) {
                let mut cli_toml = load_or_init_cli_toml(&settings)?;
                cli_toml[&settings.cli_token_key] =
                    Item::Value(profile_to_switch.token.clone().into());
                cli_toml["default_host"] = Item::Value(profile_to_switch.address.clone().into());
                update_cli_server_target(
                    &mut cli_toml,
                    &profile_name_to_switch,
                    &profile_to_switch.address,
                );
                sync_server_configs_from_profiles(&mut cli_toml, &profiles);
                write_cli_toml(&settings, &cli_toml)?;
                println!(
                    "Switched active profile to '{}' (from {}) in {}.",
                    profile_name_to_switch,
                    settings.profiles_filename,
                    settings.cli_config_filename
                );
            } else {
                println!(
                    "Profile '{}' not found in {}. Cannot switch.", // Renamed
                    profile_name_to_switch,
                    settings.profiles_filename // Renamed
                );
                println!("Available profiles: {:?}", profiles.0.keys()); // Renamed
                anyhow::bail!("Profile not found in profiles file for switching.");
                // Renamed
            }
        }
        Commands::Admin => {
            let admin_profile_name = "admin".to_string();
            let profiles = read_profiles(&settings)?;
            if let Some(admin_profile) = profiles.0.get(&admin_profile_name) {
                let mut cli_toml = load_or_init_cli_toml(&settings)?;
                cli_toml[&settings.cli_token_key] = Item::Value(admin_profile.token.clone().into());
                cli_toml["default_host"] = Item::Value(admin_profile.address.clone().into());
                update_cli_server_target(
                    &mut cli_toml,
                    &admin_profile_name,
                    &admin_profile.address,
                );
                sync_server_configs_from_profiles(&mut cli_toml, &profiles);
                write_cli_toml(&settings, &cli_toml)?;
                println!(
                    "Switched active profile to ADMIN '{}' (from {}) in {}.",
                    admin_profile_name, settings.profiles_filename, settings.cli_config_filename
                );
            } else {
                println!(
                    "ADMIN profile ('{}') not found in {}. Cannot switch.", // Renamed
                    admin_profile_name,
                    settings.profiles_filename // Renamed
                );
                println!("Ensure a profile named 'admin' exists with a valid token."); // Renamed
                anyhow::bail!("Admin profile not found."); // Renamed
            }
        }
        Commands::Save(args) => {
            let cli_toml_path = get_cli_toml_path(&settings)?;
            if !cli_toml_path.exists() {
                anyhow::bail!(
                    "{} does not exist. Cannot save token.",
                    settings.cli_config_filename
                );
            }
            let cli_toml = read_cli_toml(&settings)?;

            let mut profiles = read_profiles(&settings)?;
            if profiles.0.contains_key(&args.profile_name) {
                anyhow::bail!("Profile '{}' already exists in {}. Use a different name or delete the existing one first.", args.profile_name, settings.profiles_filename);
            }

            match (
                cli_toml.get(&settings.cli_token_key),
                cli_toml.get("default_host"),
            ) {
                (Some(token_item), Some(host_item)) => {
                    if let (Some(token_str), Some(host_str)) =
                        (token_item.as_str(), host_item.as_str())
                    {
                        let profile = Profile {
                            token: token_str.to_string(),
                            address: host_str.to_string(),
                        };
                        profiles.0.insert(args.profile_name.clone(), profile);
                        write_profiles(&settings, &profiles)?;
                        println!(
                            "Saved current active session as profile '{}' in {}.",
                            args.profile_name, settings.profiles_filename
                        );
                    } else {
                        anyhow::bail!(
                            "Token or host in {} are not strings.",
                            settings.cli_config_filename
                        );
                    }
                }
                (Some(_), None) => {
                    anyhow::bail!(
                        "'default_host' not found in {}. Cannot save profile.",
                        settings.cli_config_filename
                    );
                }
                (None, _) => {
                    anyhow::bail!(
                        "User is not logged in. Token key '{}' not found in {}.",
                        settings.cli_token_key,
                        settings.cli_config_filename
                    );
                }
            }
        }
        Commands::Reset(args) => {
            if !args.force {
                let confirmation = dialoguer::Confirm::new()
                    .with_prompt(format!(
                        "Are you sure you want to reset {}? This will delete all profiles.",
                        settings.profiles_filename
                    ))
                    .interact()?;
                if !confirmation {
                    println!("Reset cancelled.");
                    return Ok(());
                }
            }
            let profiles = UserProfiles::default();
            write_profiles(&settings, &profiles)?;
            println!("{} has been reset.", settings.profiles_filename);
        }
        Commands::Create(args) => {
            let mut profiles = read_profiles(&settings)?; // Renamed
            if profiles.0.contains_key(&args.profile_name) {
                // Renamed
                anyhow::bail!(
                    "Profile '{}' already exists in {}. Cannot create.", // Renamed
                    args.profile_name,                                   // Renamed
                    settings.profiles_filename                           // Renamed
                );
            }

            run_external_command(SPACETIME_CLI_COMMAND, &["logout"])
                .context("Failed to logout from SpacetimeDB CLI.")?;

            let address = args.address.unwrap_or_else(|| "local".to_string());
            let token = if address == "local" {
                println!(
                    "Please follow the prompts from 'spacetime login --server-issued-login {}'",
                    address
                );
                run_external_command(
                    SPACETIME_CLI_COMMAND,
                    &["login", "--server-issued-login", &address],
                )
                .with_context(|| {
                    format!(
                        "Failed during 'spacetime login --server-issued-login {}'",
                        address
                    )
                })?;

                let cli_toml_path = get_cli_toml_path(&settings)?;
                if !cli_toml_path.exists() {
                    anyhow::bail!(
                        "{} does not exist after login. Cannot save token.",
                        settings.cli_config_filename
                    );
                }
                let cli_toml = read_cli_toml(&settings)?;
                let token_item = cli_toml.get(&settings.cli_token_key).ok_or_else(|| {
                    anyhow::anyhow!(
                        "Token key '{}' not found in {} after login.",
                        settings.cli_token_key,
                        settings.cli_config_filename
                    )
                })?;
                token_item
                    .as_str()
                    .map(|value| value.to_string())
                    .ok_or_else(|| {
                        anyhow::anyhow!(
                            "Token key '{}' in {} is not a string after login.",
                            settings.cli_token_key,
                            settings.cli_config_filename
                        )
                    })?
            } else {
                fetch_server_issued_token(&address)?
            };

            let new_profile = Profile {
                token: token.clone(),
                address: address.clone(),
            };
            profiles.0.insert(args.profile_name.clone(), new_profile);
            write_profiles(&settings, &profiles)?;

            let mut cli_toml = load_or_init_cli_toml(&settings)?;
            cli_toml[&settings.cli_token_key] = Item::Value(token.into());
            cli_toml["default_host"] = Item::Value(address.clone().into());
            update_cli_server_target(&mut cli_toml, &args.profile_name, &address);
            sync_server_configs_from_profiles(&mut cli_toml, &profiles);
            write_cli_toml(&settings, &cli_toml)?;

            println!(
                "Successfully created and saved profile '{}' in {}.",
                args.profile_name, settings.profiles_filename
            );
        }
        Commands::List(args) => {
            let profiles = read_profiles(&settings)?;
            let mut active_token_opt: Option<String> = None;
            let current_env = if args.env {
                get_current_environment(&settings).context("Failed to get current environment.")?
            } else {
                None
            };

            if let Ok(cli_toml_path) = get_cli_toml_path(&settings) {
                if cli_toml_path.exists() {
                    if let Ok(cli_toml_doc) = read_cli_toml(&settings) {
                        if let Some(token_item) = cli_toml_doc.get(&settings.cli_token_key) {
                            if let Some(token_str) = token_item.as_str() {
                                active_token_opt = Some(token_str.to_string());
                            }
                        }
                    }
                }
            }

            let mut profiles_to_display = profiles.0.clone();
            if let Some(env) = &current_env {
                println!("Current environment: {}", env);
                profiles_to_display.retain(|_, profile| &profile.address == env);
            }

            if profiles_to_display.is_empty() {
                println!("No profiles found in {}.", settings.profiles_filename);
            } else {
                println!("Available profiles in {}:", settings.profiles_filename);
                let mut sorted_profile_names: Vec<_> = profiles_to_display.keys().collect();
                sorted_profile_names.sort();

                for profile_name in sorted_profile_names {
                    if let Some(profile) = profiles_to_display.get(profile_name) {
                        let mut display_name =
                            format!("- {} (address: {})", profile_name, profile.address);
                        if let Some(ref active_token) = active_token_opt {
                            if &profile.token == active_token {
                                display_name.push_str(" (current)");
                            }
                        }
                        println!("{}", display_name);
                    }
                }
            }
        }
        Commands::Current => {
            let cli_toml_path = get_cli_toml_path(&settings)?;
            if !cli_toml_path.exists() {
                println!(
                    "{} not found. No active token set.",
                    settings.cli_config_filename
                );
                return Ok(());
            }
            let cli_toml_doc = read_cli_toml(&settings)?;
            if let Some(token_item) = cli_toml_doc.get(&settings.cli_token_key) {
                if let Some(active_token_str) = token_item.as_str() {
                    let profiles = read_profiles(&settings)?;
                    let mut current_profile: Option<(String, Profile)> = None;
                    for (profile_name, profile) in profiles.0.iter() {
                        if profile.token == active_token_str {
                            current_profile = Some((profile_name.clone(), profile.clone()));
                            break;
                        }
                    }

                    if let Some((name, profile)) = current_profile {
                        println!("Current active profile: {}", name);
                        println!("Address: {}", profile.address);
                    } else {
                        println!(
                            "Current active token is set, but not found under any profile name in {}.", // Renamed
                            settings.profiles_filename // Renamed
                        );
                    }
                    println!("Active token: {}", mask_token(active_token_str));
                } else {
                    println!(
                        "Active token key '{}' in {} is not a string.",
                        settings.cli_token_key, settings.cli_config_filename
                    );
                }
            } else {
                println!(
                    "No active token (key '{}') found in {}.",
                    settings.cli_token_key, settings.cli_config_filename
                );
            }
        }
        Commands::Delete(args) => {
            let mut profiles = read_profiles(&settings)?;
            if !profiles.0.contains_key(&args.profile_name) {
                println!(
                    "Profile '{}' not found in {}. Nothing to delete.",
                    args.profile_name, settings.profiles_filename
                );
                anyhow::bail!("Profile not found for deletion.");
            }

            if !args.force {
                let confirmation = dialoguer::Confirm::new()
                    .with_prompt(format!(
                        "Are you sure you want to delete the profile '{}'?",
                        args.profile_name
                    ))
                    .interact()?;
                if !confirmation {
                    println!("Deletion cancelled.");
                    return Ok(());
                }
            }

            if profiles.0.remove(&args.profile_name).is_some() {
                write_profiles(&settings, &profiles)?;
                println!(
                    "Profile '{}' deleted from {}.",
                    args.profile_name, settings.profiles_filename
                );
            }
        }
        Commands::Env(args) => match args.command.unwrap_or(EnvCommands::Current) {
            EnvCommands::Current => match get_current_environment(&settings) {
                Ok(Some(env)) => println!("Current environment: {}", env),
                Ok(None) => println!("Environment not set."),
                Err(e) => anyhow::bail!("Failed to get current environment: {}", e),
            },
            EnvCommands::List => {
                let profiles = read_profiles(&settings)?;
                let mut env_map: BTreeMap<String, Vec<String>> = BTreeMap::new();
                for (name, profile) in profiles.0.iter() {
                    env_map
                        .entry(profile.address.clone())
                        .or_default()
                        .push(name.clone());
                }

                let current_env = get_current_environment(&settings)?;
                if env_map.is_empty() {
                    println!(
                        "No environments found. Add profiles to {} first.",
                        settings.profiles_filename
                    );
                } else {
                    println!("Known environments:");
                    for (env, mut names) in env_map {
                        names.sort();
                        let current_tag = if current_env.as_ref() == Some(&env) {
                            " (current)"
                        } else {
                            ""
                        };
                        println!("- {}{} [profiles: {}]", env, current_tag, names.join(", "));
                    }
                }
            }
            EnvCommands::Use(use_args) => {
                let profiles = read_profiles(&settings)?;
                let chosen_profile = if let Some(profile_name) = use_args.profile.clone() {
                    let profile = profiles
                        .0
                        .get(&profile_name)
                        .cloned()
                        .context(format!("Profile '{}' not found.", profile_name))?;

                    if profile.address != use_args.address {
                        anyhow::bail!(
                            "Profile '{}' uses address '{}' which does not match '{}'.",
                            profile_name,
                            profile.address,
                            use_args.address
                        );
                    }
                    (profile_name, profile)
                } else {
                    let matching_profiles: Vec<(String, Profile)> = profiles
                        .0
                        .iter()
                        .filter(|(_, profile)| profile.address == use_args.address)
                        .map(|(name, profile)| (name.clone(), profile.clone()))
                        .collect();

                    match matching_profiles.len() {
                        0 => {
                            anyhow::bail!(
                                "No profiles found for environment '{}'. Create one before switching.",
                                use_args.address
                            );
                        }
                        1 => matching_profiles[0].clone(),
                        _ => {
                            let profile_names: Vec<String> = matching_profiles
                                .iter()
                                .map(|(name, _)| name.clone())
                                .collect();
                            let selection = Select::with_theme(&ColorfulTheme::default())
                                .with_prompt("Select a profile for this environment")
                                .items(&profile_names)
                                .default(0)
                                .interact_opt()?
                                .context("No profile selected or selection cancelled.")?;

                            matching_profiles[selection].clone()
                        }
                    }
                };

                let mut cli_toml = load_or_init_cli_toml(&settings)?;
                let (profile_name, profile) = chosen_profile;
                cli_toml["default_host"] = Item::Value(profile.address.clone().into());
                cli_toml[&settings.cli_token_key] = Item::Value(profile.token.clone().into());
                update_cli_server_target(&mut cli_toml, &profile_name, &profile.address);
                sync_server_configs_from_profiles(&mut cli_toml, &profiles);
                write_cli_toml(&settings, &cli_toml)?;
                println!(
                    "Environment set to '{}' and switched to profile '{}'.",
                    profile.address, profile_name
                );
            }
        },
        Commands::SetAddress(args) => {
            let mut profiles = read_profiles(&settings)?;
            if let Some(profile) = profiles.0.get_mut(&args.profile_name) {
                let previous_address = profile.address.clone();
                let profile_token = profile.token.clone();
                profile.address = args.address.clone();
                let _ = profile;

                write_profiles(&settings, &profiles)?;
                println!(
                    "Updated address for profile '{}' to '{}'.",
                    args.profile_name, args.address
                );

                if let Ok(cli_toml_path) = get_cli_toml_path(&settings) {
                    if cli_toml_path.exists() {
                        let mut cli_toml = read_cli_toml(&settings)?;
                        let active_token_matches = cli_toml
                            .get(&settings.cli_token_key)
                            .and_then(|item| item.as_str())
                            .map(|token| token == profile_token)
                            .unwrap_or(false);
                        let host_matches = cli_toml
                            .get("default_host")
                            .and_then(|item| item.as_str())
                            .map(|host| host == previous_address)
                            .unwrap_or(false);
                        if active_token_matches || host_matches {
                            cli_toml["default_host"] = Item::Value(args.address.clone().into());
                            update_cli_server_target(&mut cli_toml, &args.profile_name, &args.address);
                            sync_server_configs_from_profiles(&mut cli_toml, &profiles);
                            write_cli_toml(&settings, &cli_toml)?;
                            println!(
                                "Updated default_host in {} to '{}'.",
                                settings.cli_config_filename, args.address
                            );
                        }
                    }
                }
            } else {
                anyhow::bail!("Profile '{}' not found.", args.profile_name);
            }
        }
        Commands::Setup => {
            let mut current_settings = load_app_settings().unwrap_or_else(|e| {
                println!(
                    "Warning: Could not load existing settings ({}). Using defaults.",
                    e
                );
                AppSettings::default()
            });

            println!("Current configuration (leave blank to keep current value):");

            let mut input = String::new();
            println!(
                "Profiles filename [{}]: ",         // Renamed
                current_settings.profiles_filename  // Renamed
            );
            std::io::stdin().read_line(&mut input)?;
            if !input.trim().is_empty() {
                current_settings.profiles_filename = input.trim().to_string(); // Renamed
            }
            input.clear();

            println!(
                "SpacetimeDB CLI config directory (from home) [{}]: ",
                current_settings.cli_config_dir_from_home
            );
            std::io::stdin().read_line(&mut input)?;
            if !input.trim().is_empty() {
                current_settings.cli_config_dir_from_home = input.trim().to_string();
            }
            input.clear();

            println!(
                "SpacetimeDB CLI config filename [{}]: ",
                current_settings.cli_config_filename
            );
            std::io::stdin().read_line(&mut input)?;
            if !input.trim().is_empty() {
                current_settings.cli_config_filename = input.trim().to_string();
            }
            input.clear();

            println!(
                "SpacetimeDB CLI token key [{}]: ",
                current_settings.cli_token_key
            );
            std::io::stdin().read_line(&mut input)?;
            if !input.trim().is_empty() {
                current_settings.cli_token_key = input.trim().to_string();
            }

            write_app_settings(&current_settings)?;
        }
    }

    Ok(())
}
