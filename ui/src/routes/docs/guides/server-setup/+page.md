---
title: Server Setup
description: Configure and manage Palworld dedicated servers
---

# Server Setup

Palworld Save Pal includes a built-in server manager for creating, configuring, and running dedicated Palworld servers directly from the app. Both **Docker** and **Native (Windows)** deployments are supported.

---

## Prerequisites {.toc}

### Hardware Requirements {.toc}

Running a Palworld dedicated server requires the following hardware ([source](https://docs.palworldgame.com/getting-started/requirements)):

| Component   | Minimum                | Recommended              |
|-------------|------------------------|--------------------------|
| **CPU**     | 4 cores                | 4+ cores                 |
| **RAM**     | 8 GB                   | 32 GB or more            |
| **Storage** | HDD (not recommended)  | SSD                      |
| **OS**      | Windows or Linux 64-bit| Windows or Linux 64-bit  |

> 8 GB of RAM is bootable but increases the possibility of server crashes due to out-of-memory errors. SSD storage is strongly recommended — low-performance storage may corrupt saved data.

### Installing Docker (Docker Deployment Only) {.toc}

If you plan to run your server using the **Docker** deployment type, you need Docker installed on the host machine.

#### Windows

1. Download [Docker Desktop for Windows](https://www.docker.com/products/docker-desktop/) and run the installer.
2. During installation, ensure **WSL 2 backend** is selected (recommended over Hyper-V).
3. After installation, restart your computer if prompted.
4. Open Docker Desktop and wait for the engine to start — the status bar should show **"Docker Desktop is running"**.
5. Open a terminal and verify the installation:
   ```
   docker --version
   ```

#### Linux

1. Follow the official install guide for your distribution:
   - [Ubuntu](https://docs.docker.com/engine/install/ubuntu/)
   - [Debian](https://docs.docker.com/engine/install/debian/)
   - [Fedora](https://docs.docker.com/engine/install/fedora/)
2. After installation, add your user to the `docker` group so you can run Docker without `sudo`:
   ```bash
   sudo usermod -aG docker $USER
   ```
   Log out and back in for the group change to take effect.
3. Start and enable the Docker service:
   ```bash
   sudo systemctl start docker
   sudo systemctl enable docker
   ```
4. Verify the installation:
   ```bash
   docker --version
   ```

### Port Forwarding {.toc}

For players outside your local network to connect, you must forward the server ports through your router.

#### Default Ports

| Port    | Protocol | Purpose                          |
|---------|----------|----------------------------------|
| `8211`  | UDP      | Game traffic (player connections) |
| `27015` | UDP      | Steam query (server browser)     |

> If you changed these ports during server creation, forward the ports you configured instead.

#### Steps

1. **Find your server's local IP address.** Open a terminal and run:
   - **Windows**: `ipconfig` — look for the **IPv4 Address** under your active network adapter.
   - **Linux**: `ip addr` — look for the `inet` address on your primary interface (e.g., `eth0` or `ens18`).
2. **Open your router's admin panel.** This is usually at `192.168.0.1` or `192.168.1.1` in a browser. Check your router's manual or the sticker on the device for the exact address and login credentials.
3. **Navigate to Port Forwarding** (may be called "Virtual Server", "NAT Forwarding", or "Applications & Gaming" depending on your router).
4. **Create forwarding rules** for each port:
   - **Service Name**: `Palworld Game` (or any label)
   - **External Port**: `8211`
   - **Internal Port**: `8211`
   - **Internal IP**: Your server's local IP from step 1
   - **Protocol**: `UDP`
   - Repeat for `27015` (Steam query port).
5. **Save and apply** the changes.
6. **Verify** the ports are open using an online port checker (search "open port check tool") or have a friend try connecting to your external IP.

> If you are behind a CGNAT (common with some ISPs), port forwarding at the router level will not work. Contact your ISP to request a public IP or consider using a VPN/tunneling service.

---

## Getting Started {.toc}

Navigate to **Servers** in the left navigation bar to open the server manager.

![Navbar Server Link](/guides/create-server/server_nav.png)

---

## Creating a Server {.toc}

1. Click the **+ New** button in the server list panel.
   ![New Button](/guides/create-server/new.png)

2. A creation modal will open with three tabs: **General**, **Gameplay**, and **Advanced**.

### General Tab {.toc}

![General Tab](/guides/create-server/create_server_general.png)

Configure the core server settings:

| Field              | Description                                                                 |
|--------------------|-----------------------------------------------------------------------------|
| **Deployment Type**| Choose **Docker** or **Native (Windows)**                                   |
| **Display Name**   | A display name shown in PSP and in the server list                          |
| **Server Name**    | A display name shown in-game                                                |
| **Server Description** | A display name shown in-game                                            |
| **Game Port**      | The port players connect to (default: `8211`)                               |
| **Query Port**     | Steam query port for server browser visibility (default: `27015`)           |
| **REST API Port**  | Port for the server's REST API used by the console and admin tools (default: `8212`) |
| **Server Password**| Optional password players must enter to join                                |
| **Admin Password** | Password for REST API authentication and in-game admin access               |
| **Max Players**    | Maximum number of concurrent players                                        |

> Ports are automatically suggested to avoid conflicts with your existing servers.

#### Docker-Specific Fields

| Field              | Description                                                    |
|--------------------|----------------------------------------------------------------|
| **Container Name** | Auto-generated from the server name; must be unique            |
| **Image Name**     | Docker image to use (default: `omanrod/psp-palworld-server`)   |

#### Native (Windows) Specific Fields

| Field               | Description                                                                 |
|----------------------|-----------------------------------------------------------------------------|
| **SteamCMD Path**    | Path to your SteamCMD directory (downloaded automatically if not present)   |
| **Install Base Path**| Directory where the Palworld dedicated server files will be installed        |
| **World Name**       | Name for the game world                                                     |
| **Launch Args**      | Additional command-line arguments passed to `PalServer.exe`                 |

### Gameplay Tab {.toc}

![Gameplay Tab](/guides/create-server/create_server_gameplay.png)

Configure in-game rates and rules, organized into groups:

- **Gameplay Rates** -- EXP rate, Pal capture rate, spawn rates, damage multipliers, hunger/stamina drain, HP regen
- **Time & Difficulty** -- Difficulty preset, day/night speed, egg hatching speed, auto-save interval, supply drop frequency
- **Stat Enhancement** -- Per-level-up stat point multipliers for HP, Attack, Stamina, Weight, Work Speed
- **PvP / Hardcore** -- PvP mode, friendly fire, hardcore mode, death penalties, aim assist
- **PvP Respawn & Rewards** -- Respawn delays, death penalties, PvP drop settings
- **Guild / Building** -- Guild size limits, base camp count, building rates, deterioration settings
- **Items & Drops** -- Drop limits, Palbox import/export, multiplayer item settings

### Advanced Tab {.toc}

![Advanced Tab](/guides/create-server/create_server_advanced.png)

Additional configuration for automation and integrations:

- **Backup Settings** -- Backup schedule (cron), retention policy
- **Auto Update / Reboot** -- Update check schedule, warning messages, reboot schedule
- **Discord Integration** -- Webhook URL for server event notifications
- **UE4SS / Mods** -- Enable UE4SS mod loader, version selection
- **Engine Settings** -- Tick rates, frame rate caps, pawn culling, container sync
- **Randomizer** -- Randomizer type, seed, random Pal levels

### Finishing Creation

Click **Create** to begin server setup.

- **Docker**: Creates the container and directory structure immediately.
- **Native**: Downloads the Palworld Dedicated Server via SteamCMD (or copies from an existing installation if one is found nearby). A progress indicator will display the current step.

<!-- TODO: screenshot of creation progress -->

---

## Server List {.toc}

Once created, your servers appear in the left panel. Each server card shows:

- **Status indicator** -- Green (running), Red (stopped), Yellow (created), Orange (paused)
- **Server name** and container/process name
- **Type badge** -- Native or Docker
- **Game port** and max player count
- **Online player count** (when running)
- **Play/Stop button** for quick start and stop

![Server List](/guides/create-server/server_list.png)

---

## Server Detail Panel {.toc}

Select a server from the list to view its details. The detail panel has five tabs:

### Overview {.toc}

![Overview Tab](/guides/create-server/server_overview.png)

Displays server information and real-time resource stats (updated every 5 seconds while running):

- **Online / Total Players**
- **Game Port** and **API Port**
- **Server Name** and **Image / Install Path**
- **CPU Usage** (%)
- **Memory Usage** (MB and %)
- **Network I/O** (Docker only)
- **Disk I/O** (read/write)

### Settings {.toc}

Edit server configuration after creation. Changes to ports, environment variables, or server identity settings will trigger a container recreation (Docker) or config rewrite and optional restart (Native).

- Server name, description, password, admin password, max players
- All environment variables grouped by category (same groups as creation)
- Click **Save** to apply changes

### Mods {.toc}

![Mods Tab](/guides/create-server/server_mods.png)

Manage server mods organized by type:

| Mod Type      | Description                                      |
|---------------|--------------------------------------------------|
| **UE4SS Lua** | Lua script mods loaded by the UE4SS mod framework |
| **Logic Mods**| `.pak` file mods placed in the LogicMods directory |
| **Native DLL**| Native DLL mods loaded at runtime                 |

- **Upload** a `.zip` file to install a new mod (select the mod type before uploading)
- **Toggle** UE4SS mods on or off using the enable/disable switch
- View the enabled/disabled/active status for each mod

### Console {.toc}

![Console Tab](/guides/create-server/server_console.png)

A built-in REST API console for direct server administration (only available while the server is running). Available endpoints:

| Endpoint        | Method | Description                          |
|-----------------|--------|--------------------------------------|
| **Server Info** | GET    | Current server information           |
| **Players**     | GET    | List of online players               |
| **Settings**    | GET    | Server configuration                 |
| **Metrics**     | GET    | Server performance metrics           |
| **Save World**  | POST   | Trigger a world save                 |
| **Shutdown**    | POST   | Graceful server shutdown             |
| **Force Stop**  | POST   | Immediately kill the server process  |
| **Announce**    | POST   | Broadcast a message to all players   |
| **Kick Player** | POST   | Kick a player by ID                  |
| **Ban Player**  | POST   | Ban a player by ID                   |
| **Unban Player**| POST   | Remove a player ban                  |

Responses are displayed in a JSON viewer with color-coded HTTP status indicators.

### Saves {.toc}

![Save Tab](/guides/create-server/server_saves.png)

Load the server's save files directly into the Palworld Save Pal editor for modification.

- Displays the save file path
- If the server is running, it will be stopped before loading
- After loading, you are navigated to the **Edit** page to begin editing

> Always back up your save files before making edits.

---

## Starting and Stopping Servers {.toc}

Use the **Play/Stop** button on the server card or the **Start/Stop** button in the detail panel header.

**Starting a server:**

- **Native**: Launches `PalServer.exe` with configured ports and launch arguments. The config file (`PalWorldSettings.ini`) is rewritten before each start to ensure settings are current.
- **Docker**: Starts the container.

**Stopping a server:**

- A graceful shutdown is attempted first via the REST API (with a short wait period).
- If the server does not stop within the timeout, the process is force-killed (Native) or the container is force-stopped (Docker).

---

## Deleting a Server {.toc}

Click the **Delete** button in the detail panel header. A confirmation dialog will appear.

- **Native**: Removes the server from the database. Server files on disk are kept by default.
- **Docker**: Removes the container and optionally its volumes.

---

## Save File Locations {.toc}

Dedicated server saves are typically found at:

```plaintext
<InstallPath>/Pal/Saved/SaveGames/0/<WorldID>/
```

### Key Files

| File                  | Description                                    |
|-----------------------|------------------------------------------------|
| `Level.sav`          | World data including buildings and map objects  |
| `Players/<UID>.sav`  | Individual player save data                    |
| `LocalData.sav`      | Local game settings                            |

---

## Tips {.toc}

- Always **stop the server** before editing save files
- Keep **regular backups** of your save directory
- Use the **Console** tab to save the world before stopping for a clean shutdown
- For native servers, the `PalWorldSettings.ini` is auto-generated from your configured settings -- edit settings through the app rather than manually editing the file
- Port conflicts are checked automatically during creation, but ensure your firewall allows traffic on the configured ports
