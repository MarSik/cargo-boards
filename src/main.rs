//! cargo-boards: Cargo subcommand to build with multiple feature configurations
//!
//! The use is mostly tailored towards embedded projects
//! where the same firmware can be used for multiple
//! boards with only minor differences.
//!
//! The tool loads the Boards.toml file and also all boards/<id>.toml files.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::string::String;
use std::vec::Vec;

use clap::Parser;

const BOARD_FILE: &str = "Boards.toml";
const CARGO_FILE: &str = "Cargo.toml";
const BOARDS_DIR: &str = "boards";

#[derive(serde::Deserialize, Clone)]
pub struct Board {
    // Unique identifier of the board
    // In case this was loaded from a file it
    // must match the filename without extension
    #[serde(skip)]
    id: String,

    // Human readable name
    #[serde(default)]
    name: String,
    // Human readable description
    #[serde(default)]
    description: String,

    // Features to enable
    #[serde(default)]
    features: Vec<String>,
    // Configs (cfg) to set
    #[serde(default)]
    configs: BTreeMap<String, String>,
    // Enable default features, defaults to true
    #[serde(default)]
    default_features: Option<bool>,
}

#[derive(serde::Deserialize)]
pub struct BoardFile {
    #[allow(dead_code)]
    #[serde(default)]
    version: Option<usize>,
    #[serde(default)]
    boards: BTreeMap<String, Board>,
}

#[derive(serde::Deserialize)]
struct CargoMetadata {
    workspace_root: String,
    target_directory: String,
}

#[derive(clap::Parser)]
#[command(version, about)]
struct Cli {
    /// Load boards from the specified file.
    #[arg(short = 'f', long)]
    board_file: Option<String>,

    /// Select one specific board.
    #[arg(short, long)]
    board: Option<String>,

    /// List all configured boards instead of building.
    #[arg(short, long)]
    list: bool,

    // The cargo command followed by any extra arguments, forwarded as-is.
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    cargo_args: Vec<String>,
}

fn fail(message: impl AsRef<str>) -> ! {
    log::error!("{}", message.as_ref());
    std::process::exit(1);
}

fn parse_board_file(path: &Path) -> Vec<Board> {
    let contents = std::fs::read_to_string(path)
        .unwrap_or_else(|e| fail(format!("failed to read {}: {e}", path.display())));
    let mut board_file: BoardFile = toml::from_str(&contents)
        .unwrap_or_else(|e| fail(format!("failed to parse {}: {e}", path.display())));
    board_file
        .boards
        .iter_mut()
        .for_each(|(k, b)| b.id = k.clone());
    board_file.boards.values().cloned().collect()
}

fn parse_boards_dir(dir: &Path) -> Vec<Board> {
    let entries = std::fs::read_dir(dir)
        .unwrap_or_else(|e| fail(format!("failed to read {}: {e}", dir.display())));

    let mut boards = Vec::new();
    for entry in entries {
        let path = entry
            .unwrap_or_else(|e| fail(format!("failed to read {}: {e}", dir.display())))
            .path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("toml") {
            continue;
        }
        let contents = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| fail(format!("failed to read {}: {e}", path.display())));
        let mut board: Board = toml::from_str(&contents)
            .unwrap_or_else(|e| fail(format!("failed to parse {}: {e}", path.display())));
        let id = path
            .file_stem()
            .unwrap_or_else(|| fail(format!("failed to get basename {path:?}")));
        board.id = id
            .to_str()
            .map(|s| s.to_string())
            .unwrap_or_else(|| fail(format!("failed to parse basename {id:?}")));
        boards.push(board);
    }
    boards
}

fn main() {
    env_logger::builder().format_timestamp(None).init();

    // When run as a cargo subcommand (`cargo boards ...`), cargo passes the
    // subcommand name "boards" as the first argument - drop it so it isn't
    // mistaken for the cargo command to forward.
    let mut args: Vec<String> = std::env::args().collect();
    if args.get(1).map(String::as_str) == Some("boards") {
        args.remove(1);
    }
    let cli = Cli::parse_from(args);

    // Find the project root and its target dir by asking cargo directly,
    // instead of walking directories ourselves - this also covers
    // workspaces or a custom target-dir configuration.
    let metadata_output = Command::new("cargo")
        .args(["metadata", "--no-deps", "--format-version", "1"])
        .output()
        .unwrap_or_else(|e| fail(format!("failed to run `cargo metadata`: {e}")));
    if !metadata_output.status.success() {
        log::error!("{}", String::from_utf8_lossy(&metadata_output.stderr));
        std::process::exit(metadata_output.status.code().unwrap_or(1));
    }
    let metadata: CargoMetadata = serde_json::from_slice(&metadata_output.stdout)
        .unwrap_or_else(|e| fail(format!("failed to parse `cargo metadata` output: {e}")));
    let workspace_root = PathBuf::from(metadata.workspace_root);
    let target_directory = PathBuf::from(metadata.target_directory);

    // Deserialize the board file(s) into a list of boards. A board file
    // passed explicitly via --board-file is the only source used; otherwise
    // both BOARD_FILE and a BOARDS_DIR directory next to CARGO_FILE are used
    // if present.
    let boards_dir = workspace_root.join(BOARDS_DIR);
    let (file_boards, dir_boards) = if let Some(board_file) = &cli.board_file {
        let path = PathBuf::from(board_file);
        if !path.exists() {
            fail(format!("board file {} does not exist", path.display()));
        }
        log::info!("info: using boards from {}", path.display());
        (parse_board_file(&path), Vec::new())
    } else {
        let board_file_path = workspace_root.join(BOARD_FILE);
        match (board_file_path.exists(), boards_dir.is_dir()) {
            (false, false) => {
                log::warn!(
                    "no {BOARD_FILE} or {BOARDS_DIR}/ found next to {CARGO_FILE}; running cargo as usual"
                );
                (Vec::new(), Vec::new())
            }
            (true, false) => {
                log::info!("using boards from {}", board_file_path.display());
                (parse_board_file(&board_file_path), Vec::new())
            }
            (false, true) => {
                log::info!("using boards from {}/", boards_dir.display());
                (Vec::new(), parse_boards_dir(&boards_dir))
            }
            (true, true) => {
                log::info!(
                    "using boards from {} and {}/",
                    board_file_path.display(),
                    boards_dir.display()
                );
                (
                    parse_board_file(&board_file_path),
                    parse_boards_dir(&boards_dir),
                )
            }
        }
    };

    // Merge datasources and sort all boards by id
    let mut all_boards: Vec<Board> = file_boards.into_iter().chain(dir_boards).collect();
    all_boards.sort_by_key(|b| b.id.clone());
    log::info!("Loaded {} boards", all_boards.len());

    // Sanitize the boards list
    // - Check that id is set.
    // - Check the same board is not defined multiple times,
    //   for example in the BOARD_FILE and BOARDS_DIR/.
    let mut errors = 0;
    let mut last_id = String::new();
    for b in &all_boards {
        if b.id.is_empty() {
            log::error!(
                "Board without id found!\nname={}\ndescription={}",
                b.name,
                b.description
            );
            errors += 1;
            continue;
        }

        // all_boards is sorted by id, if the previous one
        // had the same id there is a conflict
        if last_id == b.id {
            log::error!("Board {} is defined multiple times!", b.id);
            errors += 1;
        }

        // Record the ID for the comparison with the next item
        last_id = b.id.clone();
    }
    if errors > 0 {
        fail(format!(
            "Some boards were defined improperly. {} errors found.",
            errors
        ));
    }

    // Board list
    // Print info about all boards (sorted) and quit.
    if cli.list {
        for (i, b) in all_boards.iter().enumerate() {
            // Extra separator line
            if i > 0 {
                println!();
            }
            print!(
                "[{}]\nname: {}\ndescription: {}\n",
                b.id, b.name, b.description
            );
        }
        return;
    }

    // Select the single board to build, or all of them when no specific
    // board is selected. Assume the vector is sorted and use binary
    // search.
    let selected: Vec<Board> = if let Some(id) = &cli.board {
        all_boards
            .binary_search_by_key(id, |b| b.id.clone())
            .ok()
            .and_then(|idx| all_boards.get(idx))
            .map(|b| vec![b.clone()])
            .unwrap_or_else(|| fail(format!("no board with ID `{id}` found")))
    } else {
        all_boards
    };

    // Read the requested cargo command from the first positional cmdline
    // argument, store the rest for the cargo call below.
    let Some((command, extra_args)) = cli.cargo_args.split_first() else {
        fail("no cargo command given");
    };

    // Some cargo commands do not make sense when multiple boards are selected
    if selected.len() > 1 {
        match command.as_str() {
            "run" | "embed" => fail(format!(
                "cargo {} can operate on single board only",
                command
            )),
            _ => (),
        }
    }

    if selected.is_empty() {
        let status = Command::new("cargo")
            .arg(command)
            .args(extra_args)
            .status()
            .unwrap_or_else(|e| fail(format!("failed to run cargo: {e}")));
        std::process::exit(status.code().unwrap_or(1));
    }

    for board in &selected {
        log::info!("Building board {}", board.name);

        // Construct the necessary macro definitions.
        let mut rustflags = std::env::var("RUSTFLAGS").unwrap_or_default();
        for (key, value) in &board.configs {
            if !rustflags.is_empty() {
                rustflags.push(' ');
            }
            rustflags.push_str(&format!(r#"--cfg {}="{}""#, key, value));
        }

        // Include board cfg with board name
        if !rustflags.is_empty() {
            rustflags.push(' ');
        }
        rustflags.push_str(&format!(r#"--cfg board="{}""#, board.id,));

        // Construct the necessary feature and cfg arguments for cargo.
        let mut cmd = Command::new("cargo");
        cmd.arg(command);

        if command != "clean" {
            if !board.features.is_empty() {
                cmd.arg("--features").arg(board.features.join(","));
            }
            if board.default_features == Some(false) {
                cmd.arg("--no-default-features");
            }
            if !rustflags.is_empty() {
                cmd.env("RUSTFLAGS", rustflags);
            }
        }

        // Construct the target dir path -
        // [dirname(CARGO_FILE)/target]/boards/<board name>
        let board_target_dir = target_directory.join(BOARDS_DIR).join(&board.id);
        cmd.arg("--target-dir").arg(&board_target_dir);

        // Add all requested extra args
        cmd.args(extra_args);

        log::debug!("running: {cmd:?}");

        // Call cargo.
        let status = cmd
            .status()
            .unwrap_or_else(|e| fail(format!("failed to run cargo for board `{}`: {e}", board.id)));
        if !status.success() {
            std::process::exit(status.code().unwrap_or(1));
        }
    }
}
