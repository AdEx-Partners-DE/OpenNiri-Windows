//! OpenNiri CLI
//!
//! Command-line interface for controlling the OpenNiri window manager.
//!
//! Commands are sent to the daemon via IPC (named pipe).

use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "openniri-cli")]
#[command(author, version, about = "Control the OpenNiri window manager")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Focus commands
    Focus {
        #[command(subcommand)]
        direction: FocusDirection,
    },
    /// Scroll the viewport
    Scroll {
        #[command(subcommand)]
        direction: ScrollDirection,
    },
    /// Move the focused column
    Move {
        #[command(subcommand)]
        direction: MoveDirection,
    },
    /// Resize the focused column
    Resize {
        /// Width delta in pixels (positive to grow, negative to shrink)
        #[arg(short, long)]
        delta: i32,
    },
    /// Set the number of visible columns
    SetColumns {
        /// Number of columns to display
        count: usize,
    },
    /// Query workspace state
    Query {
        #[command(subcommand)]
        what: QueryType,
    },
    /// Reload configuration
    Reload,
    /// Stop the daemon
    Stop,
}

#[derive(Subcommand)]
enum FocusDirection {
    /// Focus the column to the left
    Left,
    /// Focus the column to the right
    Right,
    /// Focus the window above (in stacked columns)
    Up,
    /// Focus the window below (in stacked columns)
    Down,
}

#[derive(Subcommand)]
enum ScrollDirection {
    /// Scroll viewport left
    Left {
        /// Pixels to scroll (default: 100)
        #[arg(short, long, default_value = "100")]
        pixels: i32,
    },
    /// Scroll viewport right
    Right {
        /// Pixels to scroll (default: 100)
        #[arg(short, long, default_value = "100")]
        pixels: i32,
    },
}

#[derive(Subcommand)]
enum MoveDirection {
    /// Move focused column left
    Left,
    /// Move focused column right
    Right,
}

#[derive(Subcommand)]
enum QueryType {
    /// Get current workspace state
    Workspace,
    /// Get focused window info
    Focused,
    /// Get all window placements
    Placements,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    // TODO: Connect to daemon via named pipe and send command

    match cli.command {
        Commands::Focus { direction } => {
            let dir = match direction {
                FocusDirection::Left => "left",
                FocusDirection::Right => "right",
                FocusDirection::Up => "up",
                FocusDirection::Down => "down",
            };
            println!("Would send: focus {}", dir);
        }
        Commands::Scroll { direction } => match direction {
            ScrollDirection::Left { pixels } => {
                println!("Would send: scroll left {} pixels", pixels);
            }
            ScrollDirection::Right { pixels } => {
                println!("Would send: scroll right {} pixels", pixels);
            }
        },
        Commands::Move { direction } => {
            let dir = match direction {
                MoveDirection::Left => "left",
                MoveDirection::Right => "right",
            };
            println!("Would send: move {}", dir);
        }
        Commands::Resize { delta } => {
            println!("Would send: resize delta={}", delta);
        }
        Commands::SetColumns { count } => {
            println!("Would send: set-columns {}", count);
        }
        Commands::Query { what } => {
            let query = match what {
                QueryType::Workspace => "workspace",
                QueryType::Focused => "focused",
                QueryType::Placements => "placements",
            };
            println!("Would send: query {}", query);
        }
        Commands::Reload => {
            println!("Would send: reload");
        }
        Commands::Stop => {
            println!("Would send: stop");
        }
    }

    println!("\nNote: IPC communication not yet implemented.");
    println!("The daemon must be running to process these commands.");

    Ok(())
}
