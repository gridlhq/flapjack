# Flapjack Dashboard

A modern web UI for managing Flapjack search indices.

## ✨ Features

- **Search & Browse**: Interactive search testing with filters, facets, and geo-search
- **Document Management**: Browse, add, edit, delete documents with Monaco Editor
- **Index Settings**: Configure 30+ index parameters through an intuitive UI
- **API Key Management**: Create and manage API keys with ACL controls
- **System Monitoring**: View tasks, replication status, and S3 snapshots
- **API Request Logger**: Capture all API requests and export as cURL bash scripts (killer feature!)
- **Dark Mode**: Built-in light/dark mode support
- **Performance**: ~98KB gzipped initial load

## 🚀 Development

### Prerequisites

- Node.js 18+
- npm or yarn

### Setup

```bash
cd dashboard
npm install
```

### Run Development Server

```bash
npm run dev
```

Opens on http://localhost:5174 with API proxied to http://localhost:7700

### Build for Production

```bash
npm run build
```

Output: `dist/` directory (served by Flapjack at `/dashboard`)

## 🏗️ Tech Stack

- **Framework**: React 18 + TypeScript
- **Build Tool**: Vite 5
- **UI Library**: Tailwind CSS + shadcn/ui
- **State Management**:
  - React Query (API state & caching)
  - Zustand (API logger & theme)
- **Special Features**:
  - Monaco Editor (VSCode-quality JSON editing)
  - Axios interceptors (API request logging)
  - Dark mode (Tailwind's dark: variant)

## 📦 Bundle Optimization

The dashboard uses code splitting to minimize initial load:

- **react-vendor.js** (51 KB gzipped): React, React DOM, React Router
- **query-vendor.js** (28 KB gzipped): React Query, Axios
- **ui-vendor.js** (~0.1 KB gzipped): Radix UI components
- **monaco.js** (lazy-loaded): Monaco Editor (~450 KB, only loads when editing)

**Total initial load**: ~98 KB gzipped (well under 200 KB target)

## 🔌 API Integration

The dashboard integrates with Flapjack's HTTP API:

- `/1/indexes` - Index management
- `/1/indexes/:name/query` - Search & browse
- `/1/indexes/:name/settings` - Settings CRUD
- `/1/keys` - API key management
- `/1/tasks/:id` - Task status
- `/health` - Health check

All API calls are logged and can be exported as cURL commands.

## 🎨 Customization

### Theme

Edit `src/globals.css` to customize the color scheme.

### API Endpoint

Default: `http://localhost:7700` (dev mode)

In production, the dashboard is served from the same origin as the API.

## 📝 Project Structure

```
dashboard/
├── src/
│   ├── components/       # React components
│   │   ├── layout/       # Header, Sidebar, ApiLogger
│   │   ├── ui/           # shadcn/ui base components
│   │   └── ...
│   ├── pages/            # Top-level page components
│   ├── hooks/            # Custom React hooks
│   ├── lib/              # Utilities, API client, types
│   ├── App.tsx           # Main app with routing
│   ├── main.tsx          # Entry point
│   └── globals.css       # Tailwind + theme styles
├── public/               # Static assets
├── package.json
├── vite.config.ts
├── tsconfig.json
└── tailwind.config.js
```

## 🧪 Testing

The dashboard can be tested against a running Flapjack instance:

1. Start Flapjack server: `cargo run --bin flapjack-server`
2. Start dashboard dev server: `npm run dev`
3. Open http://localhost:5174

## 🚢 Deployment

The dashboard is automatically served by the Flapjack binary when built:

1. Build dashboard: `npm run build` (or use `../scripts/build-dashboard.sh`)
2. Build Rust server: `cargo build --release`
3. Run: `./target/release/flapjack-server`
4. Access dashboard at: `http://localhost:7700/dashboard`

## 📊 Bundle Analysis

View detailed bundle analysis:

```bash
npm run build
open dist/stats.html
```

## 🛠️ Development Tools

- **TypeScript**: Full type safety
- **ESLint**: Code linting
- **Vite**: Fast HMR & builds
- **Tailwind IntelliSense**: VS Code extension recommended

## 🤝 Contributing

This dashboard is part of the Flapjack project. See the main README for contribution guidelines.

## 📄 License

MIT
