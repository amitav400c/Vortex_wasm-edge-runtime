# Contributing to Vortex Wasm Edge

Thank you for your interest in contributing to Vortex!

## Prerequisites

-   Rust (latest stable)
-   bpf-linker (`cargo install bpf-linker`)

## Development Workflow

We follow a **Hybrid GitHub Flow**.

1.  **Fork** the repository (if you are an external contributor).
2.  **Clone** your fork.
3.  **Create a feature branch** from `dev`:
    ```bash
    git checkout dev
    git pull origin dev
    git checkout -b feature/my-new-feature
    ```
4.  **Make your changes**.
5.  **Run tests**:
    ```bash
    cargo test
    ```
6.  **Format your code**:
    ```bash
    cargo fmt
    ```
7.  **Push** your branch and open a **Pull Request** to the `dev` branch.

## Code Style

-   We use `rustfmt` for code styling. Please run `cargo fmt` before committing.
-   Ensure no warnings are present if possible (`cargo clippy`).

## Pull Request Process

1.  Update the `README.md` with details of changes to the interface, this includes new environment variables, exposed ports, useful file locations and container parameters.
2.  You may merge the Pull Request in once you have the sign-off of at least one other developer.
