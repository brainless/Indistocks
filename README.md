# Indistocks

A cross-platform desktop GUI application for viewing and analyzing historical data for Indian stocks from NSE (National Stock Exchange).

![Downloaded Stocks data shown in grid](/assets/screenshots/pre_mvp/pre_MVP_Stocks_Grid.png "Downloaded Stocks data shown in grid")

## Features

### Core Features
- **Cross-Platform GUI**: Native desktop application built with Rust and egui - works on Linux, Windows, and macOS
- **Fully Local**: All data stored and processed locally - no cloud dependencies or privacy concerns
- **NSE Stock List Management**: Download and maintain up-to-date list of NSE stocks with ISIN mapping
- **Historical Data Downloader**: Automated BhavCopy data downloader from NSE archives with progress tracking
- **Stock Data Grid**: Virtual scrolling table displaying all stocks with:
  - Real-time filtering by price range
  - Configurable date ranges (Last 5 Days, Last 30 Days, Last 52 Weeks)
  - Key metrics: LTP (Last Traded Price), % Change, Volume, High/Low ranges
  - Color-coded price changes (green for gains, red for losses)
  - Efficient rendering for thousands of stocks
- **Interactive Stock Charts**:
  - Historical price visualization with egui_plot
  - Adaptive date formatting based on time range
  - Hover tooltips showing exact price and date
  - Clean, responsive interface
- **Smart Search**: Fast symbol search with caching for quick access to any stock
- **Recently Viewed**: Quick access sidebar for your most recent stock views
- **SQLite Database**: Efficient local storage with proper indexing and foreign key constraints

![Graph of a single Stock from NSE India data](/assets/screenshots/pre_mvp/pre_MVP_Stock_Graph.png "Graph of a single Stock from NSE India data")

### Technical Features
- Modern Rust architecture with workspace structure
- Async download with progress reporting
- Automatic data validation and integrity checks
- Configurable data directory in user config folder
- Rate-limited API requests to respect NSE servers
- CSV parsing with flexible field handling for varying NSE formats

## Installation

### Download Pre-built Binary

Download the latest release for your platform from the [GitHub Releases](https://github.com/brainless/Indistocks/releases) page.

**Linux:**
```bash
# Download the latest release
wget https://github.com/brainless/Indistocks/releases/latest/download/indistocks-linux-x86_64.tar.gz

# Extract
tar -xzf indistocks-linux-x86_64.tar.gz

# Run
./indistocks
```

**Windows:**
Download `indistocks-windows-x86_64.zip`, extract, and run `indistocks.exe`

**macOS:**
Coming soon

### Build from Source

#### Prerequisites
- Rust toolchain (2021 edition or later)
- Display server (X11 or Wayland on Linux)

#### Build Steps
```bash
# Clone the repository
git clone https://github.com/brainless/Indistocks.git
cd Indistocks

# Build release binary
cargo build --release

# Run
cargo run --release
```

## Usage

![Home screen right after downloading](/assets/screenshots/pre_mvp/pre_MVP_Home.png "Home screen right after downloading")

### First Time Setup
1. **Download NSE Stock List**: Go to Settings (⚙ icon) and click "Download NSE List" to get the current list of NSE stocks
2. **Download Historical Data**: Click "Download BhavCopy Data" to fetch historical stock prices
   - Data downloads from yesterday backwards for ~365 days
   - Progress is shown in real-time
   - Downloaded data is automatically processed and indexed

![Settings page and download options](/assets/screenshots/pre_mvp/pre_MVP_Settings.png "Settings page and download options")

### Working with Stocks
1. **Search for Stocks**: Use the search bar at the top to find any NSE stock by symbol
2. **View Stock Charts**: Click any stock symbol to view its historical price chart
3. **Browse All Stocks**: Navigate to the "Stocks" page to see the complete grid with filters
4. **Filter by Price**: Enter min/max price range to narrow down stocks
5. **Change Time Range**: Select different ranges (5 days, 30 days, 52 weeks) to see different metrics

### Data Storage
- **Database**: `~/.config/Indistocks/db.sqlite3` (Linux) or equivalent on Windows/Mac
- **Downloads**: `~/.config/Indistocks/downloads/` organized by year/month
- **Logs**: `~/.config/Indistocks/logs/`

## Project Structure

```
Indistocks/
├── indistocks-gui/        # GUI application (eframe/egui)
│   └── src/
│       ├── main.rs        # Entry point
│       ├── app.rs         # Application state
│       └── ui/            # UI components
│           ├── main_content.rs  # Chart viewer
│           ├── stocks.rs        # Data grid
│           ├── sidebar.rs       # Recently viewed
│           ├── settings.rs      # Settings page
│           └── top_nav.rs       # Search bar
└── indistocks-db/         # Database library
    └── src/
        ├── db/            # Database operations
        │   ├── schema.rs       # Table definitions
        │   ├── operations.rs   # CRUD operations
        │   └── downloads.rs    # Download manager
        └── models/        # Data models
```

## Acknowledgments

This project is built on the shoulders of giants. Special thanks to:

### Core Technologies
- **[Rust](https://www.rust-lang.org/)** - The systems programming language that makes this possible
- **[egui](https://github.com/emilk/egui)** & **[eframe](https://github.com/emilk/egui/tree/master/crates/eframe)** - Immediate mode GUI framework for beautiful cross-platform UIs
- **[egui_plot](https://github.com/emilk/egui/tree/master/crates/egui_plot)** - Plotting library for interactive charts
- **[SQLite](https://www.sqlite.org/)** & **[rusqlite](https://github.com/rusqlite/rusqlite)** - Embedded database for local data storage

### Data & Networking
- **[reqwest](https://github.com/seanmonstar/reqwest)** - HTTP client for downloading NSE data
- **[tokio](https://tokio.rs/)** - Async runtime for concurrent operations
- **[csv](https://github.com/BurntSushi/rust-csv)** - CSV parsing for NSE data files
- **[zip](https://github.com/zip-rs/zip)** - ZIP archive extraction

### Utilities
- **[chrono](https://github.com/chronotope/chrono)** - Date and time handling
- **[serde](https://serde.rs/)** - Serialization framework
- **[directories](https://github.com/dirs-dev/directories-rs)** - Platform-specific directory paths
- **[clap](https://github.com/clap-rs/clap)** - Command-line argument parsing

### Development Tools
- **[Claude Code](https://claude.com/claude-code)** - Agentic coding assistant that helped with architecture, implementation, and testing
- **[OpenCode](https://github.com/OpenCode-ai/opencode)** - Agentic coding tool for development assistance

### Data Source
- **[NSE India](https://www.nseindia.com/)** - National Stock Exchange of India for historical market data

## License

See LICENSE file for details.
