# Indistocks

A cross-platform desktop GUI application for viewing historical data for India stocks (NSE and BSE).

## Features

- **Modern UI**: Built with egui/eframe for a responsive cross-platform experience
- **Stock Management**: Add and manage NSE stock symbols
- **Recently Viewed**: Quick access to recently viewed stocks
- **SQLite Database**: Local storage with foreign key constraints
- **Settings Page**: Configure NSE symbols with validation
- **Logs Integration**: Dedicated logs directory in user config

## Project Structure

```
src/
├── main.rs              # Entry point, eframe setup
├── app.rs               # Main App struct and state
├── ui/
│   ├── mod.rs
│   ├── top_nav.rs       # Top navigation component
│   ├── sidebar.rs       # Left sidebar component
│   ├── main_content.rs  # Main content area
│   └── settings.rs      # Settings view
├── db/
│   ├── mod.rs
│   ├── schema.rs        # Database schema
│   └── operations.rs    # CRUD operations
└── models/
    └── mod.rs           # Data models
```

## Database

- **Location**: `~/.config/Indistocks/db.sqlite3` (Linux) or equivalent on Windows/Mac
- **Logs**: `~/.config/Indistocks/logs/`
- **Tables**:
  - `nse_symbols`: NSE stock symbols
  - `bse_symbols`: BSE stock symbols
  - `recently_viewed`: Recently viewed stocks

## Building and Running

### Prerequisites

- Rust toolchain (2021 edition or later)
- Display server (X11 or Wayland on Linux)

### Build

```bash
cargo build --release
```

### Run

```bash
cargo run
```

## Usage

1. **Home Screen**: View recently viewed stocks in the left sidebar
2. **Search**: Use the search bar in the top navigation (coming soon)
3. **Settings**: Click the ⚙ icon in the top right
   - Add NSE symbols (comma-separated)
   - Invalid symbols will be shown below the textarea
4. **Logs**: Access logs from the Settings page

## UI Layout

- **Window Size**: 1200x800 (default), minimum 800x600
- **Top Navigation**: 60px height with search bar and action buttons
- **Left Sidebar**: 250px fixed width
- **Main Content**: Responsive central area

## Dependencies

- `eframe 0.29` - Cross-platform GUI framework
- `egui 0.29` - Immediate mode GUI library
- `rusqlite 0.32` - SQLite database
- `directories 5.0` - Platform-specific directories
- `chrono 0.4` - Date/time handling

## License

See LICENSE file for details.
