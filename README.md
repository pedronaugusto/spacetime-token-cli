# Spacetime Token CLI

A command-line tool to manage SpacetimeDB authentication tokens by synchronizing them between a local `profiles.toml` file (storing named profiles) and the SpacetimeDB CLI's `cli.toml` configuration.

It can be invoked as `spacetime-token` or its alias `stt`.

## Features

- **Set Profile Token**: Updates the `spacetimedb_token` in `~/.config/spacetime/cli.toml` with a token from a named profile in `profiles.toml`.
- **Save Profile Token**: Saves the current `spacetimedb_token` from `~/.config/spacetime/cli.toml` to a new named profile in `profiles.toml`. Errors if the profile name already exists.
- **Create Profile**: Initiates `spacetime logout` then `spacetime login`, and saves the new token to a named profile in `profiles.toml`. Errors if the profile name already exists.
- **List Profiles**: Lists all profile names stored in `profiles.toml`.
- **Delete Profile**: Removes a specified profile from `profiles.toml`.
- **Reset Profiles**: Clears all profiles from `profiles.toml`.
- **Switch Profile**: Switches the active token to a stored profile. If no profile name is provided, it interactively prompts for a selection from available profiles, with optional filtering by environment.
- **Admin Switch**: A dedicated command (`admin`) to quickly switch to a profile named "admin".
- **Current Profile**: Displays the currently active token and its associated profile name, if any.
- **Environment Management**: List environments derived from profiles and set the active environment (server address), optionally switching to a matching profile in one step.
- **Setup**: Interactively configure tool settings.

## Configuration

The tool uses a standard configuration directory: `~/.config/spacetime-token/` (on Linux/macOS).
If this directory or the files within it do not exist, they will be created with default values when the tool is first run or when the `setup` command is called.

1.  **`config.toml`** (located in `~/.config/spacetime-token/config.toml`):
    This file configures the behavior of the tool. You can customize these settings using the `spacetime-token setup` (or `stt setup`) command.

    ```toml
    # Configuration for the Spacetime Token CLI tool

    # Name of the TOML file storing user profiles
    profiles_filename = "profiles.toml"

    # Path to the SpacetimeDB CLI config directory, relative to the user's home directory
    cli_config_dir_from_home = ".config/spacetime"

    # Filename of the SpacetimeDB CLI configuration file
    cli_config_filename = "cli.toml"

    # Key for the token within the SpacetimeDB CLI configuration file
    cli_token_key = "spacetimedb_token"
    ```

2.  **`profiles.toml`** (located by default in `~/.config/spacetime-token/profiles.toml`; filename is configurable via `profiles_filename` in `config.toml`):
    This TOML file stores your named profiles and their corresponding tokens.
    Example:
    ```toml
    admin = "token_for_admin_profile"
    dev_profile = "token_for_dev_profile"
    ```
    If this file doesn't exist when an operation requires it, it will be created (typically empty, or populated by `create` or `save`).

## Prerequisites

- Rust and Cargo installed.
- SpacetimeDB CLI installed (for `cli.toml` interaction).

## Build

Navigate to the project directory and run:

```bash
cargo build
```

The executables will be located at `target/debug/spacetime-token` and `target/debug/stt`.

## Installation

To make the `spacetime-token` and `stt` commands available system-wide (recommended):

```bash
cargo install --path .
```

Ensure that `~/.cargo/bin` is in your system's `PATH` environment variable. After installation, you can run commands like `spacetime-token list` or `stt setup` from any terminal location.

The configuration files (`config.toml` and `profiles.toml`) will be automatically managed in the `~/.config/spacetime-token/` directory. You do not need to manually copy them after installation.
If you run the tool for the first time after installation and these files don't exist, they (and the directory) will be created with default settings. You can then use `spacetime-token setup` to customize the configuration.

## Usage

After building and installing the tool (see 'Build' and 'Installation' sections above), you can run the tool directly using the `spacetime-token` or `stt` command from any terminal location.

Use `spacetime-token help` (or `stt help`) to see a list of all commands and their descriptions.

### Commands

#### 1. `set` - Save/Update Profile and Set Active

Saves a new profile or updates an existing profile's token in `profiles.toml`, and then sets this profile's token as active in `cli.toml`.

```bash
spacetime-token set <PROFILE_NAME> <TOKEN>
# or
stt set <PROFILE_NAME> <TOKEN>
```

Example:
To save a new token for "dev_profile" (or update it if "dev_profile" already exists) and make it active:

```bash
spacetime-token set dev_profile "your_new_or_updated_dev_profile_token_here"
```

This command always requires both a profile name and a token. It will update `spacetimedb_token` in `~/.config/spacetime/cli.toml`. If `cli.toml` or its parent directories do not exist, they will be created.

#### 2. `switch` - Switch Active Profile

Looks up `<PROFILE_NAME>` in `profiles.toml` and updates `cli.toml` to use its token, making it the active profile.
If `<PROFILE_NAME>` is omitted, it will present an interactive menu to select from available profiles (all by default). Use `--address <addr>` to filter the menu to a specific environment.

```bash
spacetime-token switch [PROFILE_NAME] [--address <ADDR>]
# or
stt switch [PROFILE_NAME] [--address <ADDR>]
```

Example (direct switch):
To set an existing stored profile "admin" as active:

```bash
spacetime-token switch admin
```

Example (interactive switch):

```bash
spacetime-token switch
# (A menu will appear to select a profile)
```

Example (switch across environments):

```bash
spacetime-token switch --address http://staging.example.com/spacetime
# Interactive selection limited to profiles pointing at that address
```

#### 3. `save` - Save Current Token to a New Profile

Saves the current token from `cli.toml` to `profiles.toml` under a new profile name.
It will error if the chosen profile name already exists in `profiles.toml`.

```bash
spacetime-token save <PROFILE_NAME>
# or
stt save <PROFILE_NAME>
```

Example:

```bash
spacetime-token save my_current_session_profile
```

This reads the `spacetimedb_token` from `~/.config/spacetime/cli.toml` and saves it under the name "my_current_session_profile" in `profiles.toml`. If the token is not found in `cli.toml`, or if "my_current_session_profile" already exists as a profile, an error will be reported.

#### 4. `create` - Create New Profile via Login

For `local`, this guides you through `spacetime logout` and then `spacetime login --server-issued-login local`, then saves the newly acquired token to `profiles.toml` (in the config directory) under the provided profile name.

For remote HTTPS hosts, the tool calls `<address>/v1/identity` directly to mint a server-issued token (avoids CLI login errors when the server requires a Content-Length header). When switching or creating a profile, the tool updates `default_server` to the profile name and keeps `server_configs` in sync with saved profiles.
It will error if the chosen profile name already exists in `profiles.toml` _before_ starting the logout/login process.

```bash
spacetime-token create <PROFILE_NAME>
# or
stt create <PROFILE_NAME>
```

Example:

```bash
spacetime-token create new_user_profile
```

This command requires the `spacetime` CLI to be installed and in your PATH.

#### 5. `list` - List Profiles

Lists all profile names currently stored in `profiles.toml`. Highlights the currently active profile by appending " (current)" if its token matches the one in `cli.toml`. Use `--env` to show only profiles that match the current environment.

```bash
spacetime-token list
# or
stt list
```

Example:

```bash
spacetime-token list
```

#### 6. `delete` - Delete Profile

Removes the specified profile from `profiles.toml`.

```bash
spacetime-token delete <PROFILE_NAME>
# or
stt delete <PROFILE_NAME>
```

Example:

```bash
spacetime-token delete old_user_profile
```

If the profile does not exist, it will report an error.

#### 7. `reset` - Reset Profiles

Clears all entries from `profiles.toml`, effectively resetting it to an empty state.

```bash
spacetime-token reset
# or
stt reset
```

Example:

```bash
spacetime-token reset
```

#### 8. `setup` - Interactive Configuration

Allows you to interactively set or update the configuration values for the tool, such as the names and locations of files it uses. These settings are stored in `~/.config/spacetime-token/config.toml`.

```bash
spacetime-token setup
# or
stt setup
```

#### 9. `current` - Show Current Active Profile

Displays the token currently active in `cli.toml` (masked for security, showing only the beginning and end). If this token is associated with a profile name in `profiles.toml`, that profile name is also displayed.

```bash
spacetime-token current
# or
stt current
```

Example:

```bash
spacetime-token current
```

#### 10. `admin` - Switch to Admin Profile

A shortcut command to quickly switch the active token to the profile named "admin".
This is equivalent to `spacetime-token switch admin`.

```bash
spacetime-token admin
# or
stt admin
```

If the "admin" profile does not exist in `profiles.toml`, an error will be reported.

#### 11. `env` - Manage Environments

Inspect or set the active environment (`default_host` in `cli.toml`) while switching to a matching profile. Environments cannot be changed unless a profile with that address is selected.

Show current environment:

```bash
spacetime-token env
# or
spacetime-token env current
```

List environments discovered from saved profiles (with the current one highlighted):

```bash
spacetime-token env list
```

Set the environment and switch to a profile that uses that address:

```bash
spacetime-token env use <ADDRESS> [--profile <PROFILE_NAME>]
# examples
spacetime-token env use local
spacetime-token env use https://prod.example.com/spacetime --profile admin
```

If multiple profiles share the chosen address, you will be prompted to pick one unless you specify `--profile`. If no profiles match the address, the command will error so you can create/point a profile first.

#### 12. `set-address` - Update a Profile's Address

Update the server address associated with a stored profile. This is useful if a server URL changes or if you want to repoint a profile to a different environment.

```bash
spacetime-token set-address <PROFILE_NAME> <ADDRESS>
# or
stt set-address <PROFILE_NAME> <ADDRESS>
```
