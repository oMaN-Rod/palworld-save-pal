---
title: Getting Started
description: Learn how to set up and use Palworld Save Pal
---

# Getting Started

Welcome to **Palworld Save Pal** — a save file editor for Palworld that lets you modify players, Pals, items, and more.

## Installation {.toc}

### Desktop App (Recommended)

1. Download the latest release from the [GitHub releases page](https://github.com/oMaN-Rod/palworld-save-pal/releases)
2. Extract the archive to a folder of your choice
3. Run the executable to launch the application

### Docker

For server environments or advanced users, a Docker image is available:

```bash
docker pull ghcr.io/oman-rod/palworld-save-pal:latest
```

## Loading a Save File {.toc}

### Desktop Mode

1. Click **File** in the navigation bar
2. Select your Palworld save directory
3. The application will parse and load your save data

### Web Mode

1. Navigate to the **Transfer** page
2. Upload your `.sav` file
3. Wait for the file to be processed

## Editing Your Save {.toc}

Once a save file is loaded, you can:

- **Edit Players** — Modify stats, inventory, and equipment
- **Edit Pals** — Change skills, stats, and attributes
- **Manage Palbox** — Organize and modify stored Pals
- **Edit Technologies** — Unlock or modify technology progress
- **Edit Guilds** — Manage guild settings and members

Navigate between sections using the keyboard shortcuts shown in the edit toolbar, or click the tabs directly.

## Saving Changes {.toc}

### Desktop Mode

Click the **Save** button in the navigation bar to write changes back to your save file. A backup is automatically created.

### Web Mode

Use the **Download** button to export your modified save file, then replace the original file on your server.

> **Important:** Always back up your save files before making modifications. While Palworld Save Pal creates automatic backups in desktop mode, it's good practice to maintain your own copies.
