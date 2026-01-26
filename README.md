# App ID Finder

A Rust-based web application that discovers iOS and Android app bundle IDs instantly.

## Features

- 🍎 **iOS Bundle IDs** - Look up bundle identifiers for iOS apps via the iTunes API
- 🤖 **Android Package Names** - Scrape Google Play Store for Android app package names
- 🔗 **Direct Store Links** - Get direct links to App Store and Google Play pages
- 🌐 **Web Interface** - Beautiful, modern web UI for easy searching
- 🚀 **REST API** - Simple API endpoints for programmatic access

## Installation

### Prerequisites

- [Rust](https://www.rust-lang.org/tools/install) (2024 edition or later)
- Cargo (included with Rust)

### Build

```bash
# Clone the repository
git clone https://github.com/rvido/appid_finder.git
cd appid_finder

# Build in release mode
cargo build --release
```

## Usage

### Running the Server

```bash
# Run the server (defaults to port 3000)
cargo run --release

# Or specify a custom port
APPID_PORT=8080 cargo run --release
```

The server will display available network URLs for LAN access.

### Web Interface

Open your browser and navigate to `http://localhost:3000` to access the web interface.

### API Endpoints

#### Search for Apps
```
GET /api/search?q={app_name}
```

Returns app information for both iOS and Android platforms.

**Example:**
```bash
curl "http://localhost:3000/api/search?q=YouTube"
```

**Response:**
```json
{
  "results": [
    {
      "name": "YouTube",
      "bundleId": "com.google.ios.youtube",
      "platform": "iOS",
      "storeUrl": "https://apps.apple.com/app/id544007664"
    },
    {
      "name": "YouTube",
      "bundleId": "com.google.android.youtube",
      "platform": "Android",
      "storeUrl": "https://play.google.com/store/apps/details?id=com.google.android.youtube"
    }
  ],
  "count": 2,
  "query": "Search results for: YouTube"
}
```

#### Health Check
```
GET /api/health
```

Returns the service status.

## Project Structure

```
appid_finder/
├── Cargo.toml          # Project dependencies and metadata
├── LICENSE             # MIT License
├── README.md           # This file
├── src/
│   ├── main.rs         # Web server and API endpoints
│   ├── ios_appid.rs    # iOS App Store API integration
│   └── android_appid.rs # Google Play Store scraping
└── web/
    ├── index.html      # Web interface
    └── styles.css      # Styling
```

## Dependencies

- **axum** - Web framework
- **tokio** - Async runtime
- **reqwest** - HTTP client
- **scraper** - HTML parsing for web scraping
- **serde** / **serde_json** - JSON serialization
- **tower-http** - HTTP middleware (CORS, static files)

## Running Tests

```bash
cargo test
```

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## Author

Richard Vidal-Dorsch

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.
