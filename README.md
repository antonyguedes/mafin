# Mafin

**A fast, precise, and offline-first personal finance & investment manager.**

Mafin is a desktop application designed to give you complete control over your personal finances and investment portfolio. Built entirely in Rust from front to back, it ensures high performance, memory safety, and absolute precision in every calculation.

## ✨ Key Features

- **Asset & Portfolio Management:** Track your stocks and financial assets. Monitor your positions, allocation slices, and portfolio performance in real-time.
- **Order Tracking:** Keep a detailed log of all your buy and sell orders, including precise quantities, prices, and brokerage fees.
- **Tax Reporting (Brazilian IR):** Automated tax calculations specifically designed to help with Brazilian Income Tax (Imposto de Renda) declarations.
- **Transaction Management:** Easily record your daily income and expenses, organized by categories.
- **Absolute Precision:** Financial math should never rely on approximations. Mafin uses `rust_decimal` across the entire stack to eliminate floating-point errors, ensuring every cent is perfectly accounted for.
- **Local & Private:** Your financial data stays on your machine. Everything is securely stored in a local SQLite database—no cloud syncing, no privacy concerns.

## 🛠️ Technology Stack

This project is structured as a Cargo Workspace utilizing modern Rust tooling:

- **Backend:** [Rust](https://www.rust-lang.org/) & [Tauri](https://v2.tauri.app/)
- **Database:** [SQLite](https://sqlite.org/) powered by [sqlx](https://github.com/launchbadge/sqlx) for compile-time checked queries.
- **Frontend:** [Leptos](https://leptos.dev/) (CSR via WebAssembly)
- **Styling:** [Tailwind CSS](https://tailwindcss.com/)
- **Bundler:** [Trunk](https://trunkrs.dev/)

## 📂 Workspace Architecture

- `/src-tauri`: The native backend containing Tauri setup, database repositories, and OS-level integrations.
- `/frontend`: The Leptos web application compiled to WebAssembly.
- `/shared`: Pure domain logic, types, and IPC (Inter-Process Communication) definitions shared between the backend and frontend.

## 🚀 Getting Started

### Prerequisites

Make sure you have the following installed on your system:
- [Rust](https://rustup.rs/) (v1.88+)
- [Trunk](https://trunkrs.dev/): `cargo install trunk`
- OS-specific dependencies for Tauri (check the [Tauri Setup Guide](https://v2.tauri.app/start/prerequisites/)).

### Running the App Locally

To start the application in development mode (which will automatically serve the frontend with Trunk and open the Tauri window):

```bash
cargo tauri dev
```

### Building for Production

To create an optimized, standalone executable:

```bash
cargo tauri build
```

## 📄 License

This project is licensed under the [MIT License](LICENSE).
