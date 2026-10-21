//! omega-ui's tests, built as one binary. Built one per file, linking and starting them took
//! longer than running them. A test that needs a process to itself stays a file of its own in
//! tests/, beside this folder: theme sets the chrome's colours, which a process sets only once.

mod backdrop;
mod battery;
mod bezel;
mod bluetooth;
mod board;
mod cart;
mod chrome;
mod clock;
mod contrast;
mod disc;
mod dive;
mod font;
mod hucard;
mod hud;
mod keyboard;
mod kit;
mod kit_hints;
mod kit_hud;
mod kit_layout;
mod label;
mod label_shrink;
mod letters;
mod link_art;
mod md_cart;
mod nes_cart;
mod network;
mod opening;
mod palette;
mod polaroids;
mod quick_menu;
mod refusal;
mod render_board;
mod render_gb_cart;
mod render_row_edges;
mod render_shelf_wrap;
mod render_snes_cart;
mod saves;
mod shelf;
mod shell;
mod spin;
mod switcher;
mod toast;
mod top_bar;

/// A file in this folder that the list above leaves out is never compiled, let alone run.
#[test]
fn every_file_in_the_folder_is_in_the_suite() {
    let folder = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/suite");
    let listed = include_str!("main.rs");
    for entry in std::fs::read_dir(&folder).expect("the suite's folder") {
        let path = entry.expect("a file in the suite's folder").path();
        let stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or_default();
        if path.extension().is_some_and(|e| e == "rs") && stem != "main" {
            assert!(
                listed.contains(&format!("\nmod {stem};")),
                "tests/suite/{stem}.rs is missing from tests/suite/main.rs"
            );
        }
    }
}
