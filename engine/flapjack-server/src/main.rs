#[cfg(not(target_env = "msvc"))]
#[global_allocator]
static GLOBAL: tikv_jemallocator::Jemalloc = tikv_jemallocator::Jemalloc;

use clap::{Parser, Subcommand};
use flapjack_http::serve;

#[derive(Parser)]
#[command(name = "flapjack")]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,

    #[arg(long, env = "FLAPJACK_DATA_DIR", default_value = "./data")]
    data_dir: String,
    #[arg(long, env = "FLAPJACK_BIND_ADDR", default_value = "127.0.0.1:7700")]
    bind_addr: String,
    #[arg(long, default_value = "7700")]
    port: u16,
}

#[derive(Subcommand)]
enum Command {
    /// Remove Flapjack binary and clean up shell PATH entries
    Uninstall,
}

fn run_uninstall() -> Result<(), Box<dyn std::error::Error>> {
    let home = std::env::var("HOME").map_err(|_| "HOME environment variable not set")?;
    let install_dir =
        std::env::var("FLAPJACK_INSTALL").unwrap_or_else(|_| format!("{}/.flapjack", home));

    // Remove the install directory
    if std::path::Path::new(&install_dir).exists() {
        std::fs::remove_dir_all(&install_dir)?;
        eprintln!("Removed {}", install_dir);
    } else {
        eprintln!("Directory {} does not exist, skipping", install_dir);
    }

    // Clean PATH entries from shell config files
    let rc_files = [
        format!("{}/.bashrc", home),
        format!("{}/.bash_profile", home),
        format!("{}/.zshrc", home),
        format!("{}/.profile", home),
        format!("{}/.config/fish/config.fish", home),
    ];

    for rc_path in &rc_files {
        let path = std::path::Path::new(rc_path);
        if !path.exists() {
            continue;
        }

        let contents = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => continue,
        };

        if !contents.contains(".flapjack") {
            continue;
        }

        // Remove the "# Flapjack" comment line and the export/set line that follows it
        let mut new_lines: Vec<&str> = Vec::new();
        let mut lines = contents.lines().peekable();
        let mut modified = false;

        while let Some(line) = lines.next() {
            if line.trim() == "# Flapjack" {
                // Skip this comment and the next line (the export/set PATH line)
                if let Some(next) = lines.peek() {
                    if next.contains(".flapjack") {
                        lines.next(); // consume the PATH line
                        modified = true;
                        // Also skip a leading blank line if we left one
                        continue;
                    }
                }
                modified = true;
                continue;
            }
            // Skip standalone PATH lines referencing .flapjack (in case format differs)
            if (line.contains("export PATH") || line.contains("set -gx PATH"))
                && line.contains(".flapjack")
            {
                modified = true;
                continue;
            }
            new_lines.push(line);
        }

        if modified {
            // Trim trailing blank lines that may have been left behind
            while new_lines.last() == Some(&"") {
                new_lines.pop();
            }
            let mut new_contents = new_lines.join("\n");
            if !new_contents.is_empty() {
                new_contents.push('\n');
            }
            std::fs::write(path, new_contents)?;
            eprintln!("Cleaned PATH entry from {}", rc_path);
        }
    }

    eprintln!("\nFlapjack has been uninstalled.");
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    match cli.command {
        Some(Command::Uninstall) => run_uninstall(),
        None => {
            std::env::set_var("FLAPJACK_DATA_DIR", &cli.data_dir);
            std::env::set_var("FLAPJACK_BIND_ADDR", &cli.bind_addr);
            serve().await
        }
    }
}
