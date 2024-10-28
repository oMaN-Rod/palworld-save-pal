# PALWORLD Save Pal

> **Note**: This project was put together for fun and to kick the tires on Sveltekit 5 and Skeleton UI Next. Things may be broken or not work as expected. 

⚠️ **Backup your save files before using this tool!!** ⚠️

Palworld Save Pal is a tool for managing and analyzing save files.

## 📋 Table of Contents

- [PALWORLD Save Pal](#palworld-save-pal)
  - [📋 Table of Contents](#-table-of-contents)
  - [🚀 Installation](#-installation)
  - [🎮 Usage](#-usage)
  - [🐳 Docker](#-docker)
  - [👨‍💻 Developer Guide](#-developer-guide)
    - [Web](#web)
    - [Build Desktop App](#build-desktop-app)
      - [Using build script](#using-build-script)
      - [Manual build](#manual-build)
  - [🗺️ Roadmap](#️-roadmap)
    - [General](#general)
    - [Pals](#pals)
    - [Players](#players)
    - [Map](#map)
  - [📜 License](#-license)
  - [➡️ Related Projects](#️-related-projects)
  - [☕ Buy me a coffee](#-buy-me-a-coffee)

## 🚀 Installation

Grab the latest release from the [releases](https://github.com/oMaN-Rod/palworld-save-pal/releases) page and extract it to a folder of your choice.

## 🎮 Usage

Details for using Palworld Save Pal can be found in the [User Guide](https://github.com/oMaN-Rod/palworld-save-pal/wiki/%F0%9F%8E%AE-Usage)

## 🐳 Docker

To run Palworld Save Pal using Docker:

1. Clone this repository:

   ```bash
   git clone https://github.com/oMaN-Rod/palworld-save-pal.git
   ```

2. Run the build script based on your environment, these scripts capture the system IP address and set the environment variable for the svelte SPA:
   > Linux
   ```bash
   ./build-docker.sh
   ```

   > Windows
   ```powershell
   .\build-docker.ps1
   ```

3. Or you can follow these steps:
   1. Set the environment variable for the svelte SPA `ui/.env`. Replace `{{ ip_address }}` with the IP address of the server::

      ```env
      PUBLIC_WS_URL={{ ip_address }}:5174/ws
      PUBLIC_DESKTOP_MODE=false
      ```

   2. Build the SPA (replace bun with your package manager of choice). This will create a build directory in the project root containing the static files for the SPA:

      ```bash
      cd ui
      rm -rf .svelte-kit
      bun install
      bun run build
      ```

   3. Build the docker image:

      ```bash
      docker-compose up --build -d
      ```

## 👨‍💻 Developer Guide

For developers who want to contribute to Palworld Save Pal:

### Web

1. Set up the development environment:

   ```bash
   python -m venv .venv
   source .venv/bin/activate
   pip install -r requirements.txt
   ```

2. Run the application in development mode:

   ```bash
   python psp.py --dev
   ```

3. Set the environment variable for the svelte SPA `ui/.env`.

   ```env
   PUBLIC_WS_URL=127.0.0.1:5174/ws
   PUBLIC_DESKTOP_MODE=false
   ```

4. Run the frontend in development mode:

   ```bash
   cd ui
   bun install
   bun run dev
   ```

5. Open your browser and navigate to `http://127.0.0.1:5173`

### Build Desktop App

> Activate the environment

```powershell
python -m venv .venv
source .\.venv\Scripts\activate
pip install -r requirements.txt
```

#### Using build script

```powershell
.\build-desktop.ps1
```

#### Manual build

1. Set the environment variable for the svelte SPA `ui/.env`.

   ```env
   PUBLIC_WS_URL=127.0.0.1:5174/ws
   PUBLIC_DESKTOP_MODE=true
   ```

2. Create EXE:

   ```powershell
   pyinstaller desktop.spec
   ```
   
3. Build the SPA (replace bun with your package manager of choice). This will create a build directory in the project root containing the static files for the SPA:

   ```powershell
   cd ui
   rm .svelte-kit
   bun run build
   ```

4. Copy build to the dist folder:

   ```powershell
   cp -R .\build\ .\dist\
   cp -R .\data\ .\dist\
   ```

> **Note:** The `dist` folder will contain the executable and the SPA build files, the data folder contains json files with game data, all need to be distributed together.

## 🗺️ Roadmap

Here's what's planned for future releases of Palworld Save Pal:

### General

- [X] Filter Pals by name, nickname, or Element type
- [ ] Remote access to save files (sftp to remote server)
- [X] Bulk edit pals (e.g., set all stomachs to 100%)

### Pals

- [X] Edit Nickname
- [X] Edit Gender
- [X] Edit Active Skills
- [X] Edit Passive Skills
- [X] Edit Health - Modified pals are automatically healed
- [X] Edit Stomach - Modified pals are automatically healed
- [X] Edit Level
- [X] Edit learned skills
- [X] Edit Rank
- [X] Edit Souls
- [X] Set/Unset Lucky
- [X] Set/Unset Boss
- [X] Add/Remove Pals
- [X] Clone Pals

### Players

- [ ] Edit Player Name
- [X] Edit Player Level
- [X] Edit Player Inventory
- [X] Player presets for inventory

### Map

- [ ] Edit Storage items

## 📜 License

MIT License (do whatever you want with it).

## ➡️ Related Projects

These are projects I've found that specifically target PALWorld save files, each was helpful in some way during the development of this project:

- [PalEdit](https://github.com/EternalWraith/PalEdit) - PSP was inspired by it.
- [palworld-save-tools](https://github.com/cheahjs/palworld-save-tools) - PSP uses this tool for handling save files, can be used directly to convert to/from json.
- [palworld-uesave-rs](https://github.com/DKingAlpha/palworld-uesave-rs) - I originally considered building this app using Tauri, opted for using Python, but this project was helpful.
- [Palworld Pal Editor](https://github.com/KrisCris/Palworld-Pal-Editor) - Also served as a reference for Palworld Save Pal, adopted some of this projects approach.

## ☕ Buy me a Coffee

[!["Buy Me A Coffee"](https://www.buymeacoffee.com/assets/img/custom_images/orange_img.png)](https://buymeacoffee.com/i_am_o)
