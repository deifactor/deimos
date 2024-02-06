pub mod app;
mod audio;
pub mod library;
mod library_panel;
mod mpris;
mod ui;

#[cfg(test)]
#[macro_export]
macro_rules! test_data {
    ($fname:expr) => {
        [env!("CARGO_MANIFEST_DIR"), "test_data", $fname].iter().collect::<PathBuf>()
    };
}
