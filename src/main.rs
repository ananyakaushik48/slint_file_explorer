use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    rc::Rc,
    sync::{mpsc, Arc, Mutex, Weak},
    thread,
    time::{SystemTime, UNIX_EPOCH},
};
use slint::{ModelRc, SharedString, VecModel};
use regex::RegexBuilder;
use std::os::unix::fs::PermissionsExt;

// Use the Slint-generated `FileEntry` from the .slint code below
use crate::slint_generatedMainWindow::FileEntry as SlintFileEntry;

slint::slint! {
    import { Button, VerticalBox, ListView } from "std-widgets.slint";

    struct Theme {
        primary: brush,
        secondary: brush,
        text: brush,
        accent: brush,
        hover: brush,
        background: brush,
        directory: brush,
        executable: brush,
        file: brush,
        hover-text: brush,
    }

    global Palette {
        out property <Theme> theme: {
            primary: #1a1a1aff,
            secondary: #2d2d2dff,
            text: #ffffffff,
            accent: #007accff,
            hover: #3d3d3dff,
            background: #000000ff,
            executable: #ce9178ff,
            directory: #ff8c00ff,
            file: #9cdcfeff,
            hover-text: #000000ff,
        };
    }

    export struct FileEntry {
        name: string,
        is_directory: bool,
        is_executable: bool,
        path: string,
        size: string,
        modified: string,
    }

    component SearchBar {
        callback search-changed(string);
        callback search-submitted();

        Rectangle {
            background: Palette.theme.secondary;
            border-radius: 4px;
            height: 36px;

            HorizontalLayout {
                padding: 8px;
                spacing: 8px;

                ti := TextInput {
                    text: "";
                    color: Palette.theme.text;
                    font-size: 14px;
                    edited => {
                        root.search-changed(self.text);
                    }
                    accepted => {
                        root.search-submitted();
                    }
                }
            }
        }
    }

    component FileItemView {
        in property <FileEntry> entry;
        callback clicked();
        callback double-clicked();

        Rectangle {
            background: ta.pressed ? Palette.theme.accent
                       : ta.has-hover ? Palette.theme.hover
                       : transparent;
            border-radius: 4px;

            ta := TouchArea {
                clicked => { root.clicked(); }
                double-clicked => { root.double-clicked(); }
            }

            HorizontalLayout {
                padding: 12px;
                spacing: 12px;

                Text {
                    text: root.entry.is_directory ? "📁"
                         : root.entry.is_executable ? "⚡"
                         : "📄";
                    font-size: 16px;
                }

                Text {
                    text: root.entry.name;
                    color: ta.has-hover ? Palette.theme.hover-text
                           : root.entry.is_directory ? Palette.theme.directory
                           : root.entry.is_executable ? Palette.theme.executable
                           : Palette.theme.file;
                    font-size: 14px;
                }

                Text {
                    text: root.entry.size;
                    color: ta.has-hover ? Palette.theme.hover-text
                           : Palette.theme.text;
                    font-size: 14px;
                }

                Text {
                    text: root.entry.modified;
                    color: ta.has-hover ? Palette.theme.hover-text
                           : Palette.theme.text;
                    font-size: 14px;
                }
            }
        }
    }

    export component MainWindow inherits Window {
        in-out property <[FileEntry]> files: [];
        in-out property <string> current-path: "/";
        in-out property <bool> is-searching: false;

        callback navigate(string);
        callback open-file(string);
        callback up-directory();
        callback go-to-root();
        callback search-text-changed(string);
        callback search-submitted();

        title: "File Explorer Pro";
        background: Palette.theme.background;
        min-width: 900px;
        min-height: 600px;

        VerticalLayout {
            padding: 16px;
            spacing: 16px;

            HorizontalLayout {
                spacing: 12px;
                height: 36px;

                VerticalBox {
                    Button {
                        text: "⬆️ Up";
                        clicked => { root.up-directory(); }
                    }
                }

                VerticalBox {
                    Button {
                        text: "🏠 Root";
                        clicked => { root.go-to-root(); }
                    }
                }

                Rectangle {
                    background: Palette.theme.secondary;
                    border-radius: 4px;
                    Text {
                        text: root.current-path;
                        color: Palette.theme.text;
                        font-size: 14px;
                        padding: 8px;
                    }
                }
            }

            SearchBar {
                search-changed(text) => {
                    root.search-text-changed(text);
                }
                search-submitted() => {
                    root.search-submitted();
                }
            }

            if root.is-searching: Rectangle {
                height: 24px;
                Text {
                    text: "🔍 Searching...";
                    color: Palette.theme.text;
                    font-size: 14px;
                }
            }

            Rectangle {
                background: Palette.theme.secondary;
                border-radius: 4px;
                clip: true;

                ListView {
                    for entry in files: FileItemView {
                        entry: entry;
                        clicked => {
                            if (entry.is_directory) {
                                root.navigate(entry.path);
                            }
                        }
                        double-clicked => {
                            if (!entry.is_directory) {
                                root.open-file(entry.path);
                            }
                        }
                    }
                }
            }
        }
    }
}

// -----------------------------------------------------------------------------
// Helper Functions
// -----------------------------------------------------------------------------

fn format_size(size: u64) -> String {
    if size < 1024 {
        format!("{}B", size)
    } else if size < 1024 * 1024 {
        format!("{:.1}KB", size as f64 / 1024.0)
    } else if size < 1024 * 1024 * 1024 {
        format!("{:.1}MB", size as f64 / (1024.0 * 1024.0))
    } else {
        format!("{:.1}GB", size as f64 / (1024.0 * 1024.0 * 1024.0))
    }
}

fn format_time(time: SystemTime) -> String {
    if let Ok(duration) = time.duration_since(UNIX_EPOCH) {
        let secs = duration.as_secs();
        let years = 1970 + (secs / 31_536_000);
        let months = ((secs % 31_536_000) / 2_592_000) + 1;
        let days = ((secs % 2_592_000) / 86_400) + 1;
        let hours = (secs % 86_400) / 3_600;
        let minutes = (secs % 3_600) / 60;
        format!("{:04}-{:02}-{:02} {:02}:{:02}", years, months, days, hours, minutes)
    } else {
        String::from("Unknown")
    }
}

fn is_executable(path: &Path) -> bool {
    #[cfg(unix)]
    {
        if let Ok(metadata) = fs::metadata(path) {
            return metadata.permissions().mode() & 0o111 != 0;
        }
    }
    #[cfg(windows)]
    {
        if let Some(ext) = path.extension() {
            return ext.eq_ignore_ascii_case("exe")
                || ext.eq_ignore_ascii_case("bat")
                || ext.eq_ignore_ascii_case("cmd")
                || ext.eq_ignore_ascii_case("com");
        }
    }
    false
}

fn create_file_entry(path: &Path, metadata: &fs::Metadata) -> SlintFileEntry {
    SlintFileEntry {
        name: SharedString::from(
            path.file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string()
        ),
        is_directory: path.is_dir(),
        is_executable: is_executable(path),
        path: SharedString::from(path.to_string_lossy().to_string()),
        size: SharedString::from(format_size(metadata.len())),
        modified: SharedString::from(format_time(
            metadata.modified().unwrap_or(SystemTime::now())
        )),
    }
}

// -----------------------------------------------------------------------------
// Pre-Order DFS Search
// -----------------------------------------------------------------------------

/// Perform a **pre-order DFS** with a stack:
///  1) For each popped directory, immediately check filenames for a match
///  2) If directory, push it on the stack
/// This approach can be faster in some cases than BFS.
fn dfs_search(root_dir: String, pattern: &regex::Regex, tx: mpsc::Sender<SlintFileEntry>) {
    let mut stack = vec![PathBuf::from(&root_dir)];

    while let Some(dir) = stack.pop() {
        // Attempt to read this directory
        let entries = match fs::read_dir(&dir) {
            Ok(e) => e,
            Err(_) => continue, // skip directories we cannot read
        };

        for entry in entries {
            let entry = match entry {
                Ok(ent) => ent,
                Err(_) => continue,
            };
            let path = entry.path();
            let filename = match path.file_name() {
                Some(fname) => fname.to_string_lossy(),
                None => continue,
            };

            // **Only** call fs::metadata if filename matches pattern
            if pattern.is_match(&filename) {
                if let Ok(md) = fs::metadata(&path) {
                    let fe = create_file_entry(&path, &md);
                    // Send result
                    let _ = tx.send(fe);
                }
            }

            // Pre-order DFS: if directory, push to stack
            if path.is_dir() {
                stack.push(path);
            }
        }
    }
}

// -----------------------------------------------------------------------------
// SearchState + Main
// -----------------------------------------------------------------------------

struct SearchState {
    query: String,
    is_searching: bool,
}

fn main() {
    let app = MainWindow::new().unwrap();
    let weak = app.as_weak();

    #[cfg(target_os = "windows")]
    let root_path = "C:\\".to_string();
    #[cfg(not(target_os = "windows"))]
    let root_path = "/".to_string();

    let current_path = Arc::new(Mutex::new(PathBuf::from(&root_path)));
    let search_state = Arc::new(Mutex::new(SearchState {
        query: String::new(),
        is_searching: false,
    }));

    // 1) Listing the immediate directory
    let list_files: Arc<dyn Fn(&MainWindow)> = {
        let current_path = Arc::clone(&current_path);
        Arc::new(move |app: &MainWindow| {
            let path = current_path.lock().unwrap();
            let mut entries = Vec::new();

            if let Ok(dir_entries) = fs::read_dir(&*path) {
                for e in dir_entries {
                    if let Ok(entry) = e {
                        let p = entry.path();
                        if let Ok(md) = fs::metadata(&p) {
                            entries.push(create_file_entry(&p, &md));
                        }
                    }
                }
            }

            // Sort directories first, then by name
            entries.sort_by(|a, b| {
                match (a.is_directory, b.is_directory) {
                    (true, false) => std::cmp::Ordering::Less,
                    (false, true) => std::cmp::Ordering::Greater,
                    _ => a.name.as_str().cmp(b.name.as_str()),
                }
            });

            app.set_files(ModelRc::from(Rc::new(VecModel::from(entries))));
        })
    };

    // 2) DFS-based searching (spawns a background thread)
    let setup_search: Arc<dyn Fn(String) + 'static> = {
        let app_weak = weak.clone();
        let search_state = Arc::clone(&search_state);
        let root_path = root_path.clone();

        Arc::new(move |query: String| {
            let app = match app_weak.upgrade() {
                Some(a) => a,
                None => return,
            };

            // If empty, no search
            if query.is_empty() {
                let mut state = search_state.lock().unwrap();
                state.is_searching = false;
                drop(state);
                app.set_is_searching(false);
                return;
            }

            {
                let mut state = search_state.lock().unwrap();
                state.is_searching = true;
            }
            app.set_is_searching(true);

            // Build the (case-insensitive) regex pattern
            let pattern = RegexBuilder::new(&regex::escape(&query))
                .case_insensitive(true)
                .build()
                .unwrap_or_else(|_| RegexBuilder::new("").build().unwrap());

            // Channel
            let (tx, rx) = mpsc::channel();
            let app_weak = app_weak.clone();
            let search_state = Arc::clone(&search_state);

            let root_path_clone = root_path.clone();

            thread::spawn(move || {
                // Pre-order DFS
                dfs_search(root_path_clone, &pattern, tx);

                // Collect matches
                let mut results = Vec::new();
                // No explicit drop(tx); automatically dropped here
                while let Ok(fe) = rx.recv() {
                    results.push(fe);
                }

                // Sort directories first, then by name
                results.sort_by(|a, b| {
                    match (a.is_directory, b.is_directory) {
                        (true, false) => std::cmp::Ordering::Less,
                        (false, true) => std::cmp::Ordering::Greater,
                        _ => a.name.as_str().cmp(b.name.as_str()),
                    }
                });

                // Update UI if alive
                if let Some(app) = app_weak.upgrade() {
                    let mut state = search_state.lock().unwrap();
                    state.is_searching = false;
                    drop(state);
                    app.set_is_searching(false);

                    // If your Slint requires the main thread, do:
                    // slint::invoke_from_event_loop(move || { app.set_files(...); });
                    app.set_files(ModelRc::from(Rc::new(VecModel::from(results))));
                }
            });
        })
    };

    // 3) Hook up UI callbacks
    {
        let setup_search = Arc::clone(&setup_search);
        let search_state = Arc::clone(&search_state);
        let weak = weak.clone();
        let list_files = Arc::clone(&list_files);

        app.on_search_text_changed(move |text: SharedString| {
            let mut state = search_state.lock().unwrap();
            state.query = text.to_string();

            if text.is_empty() {
                state.is_searching = false;
                drop(state);
                let app = match weak.upgrade() {
                    Some(a) => a,
                    None => return,
                };
                app.set_is_searching(false);
                list_files(&app);
            } else {
                (setup_search)(state.query.clone());
            }
        });
    }

    {
        let setup_search = Arc::clone(&setup_search);
        let search_state = Arc::clone(&search_state);

        app.on_search_submitted(move || {
            if let Ok(state) = search_state.lock() {
                if !state.query.is_empty() {
                    (setup_search)(state.query.clone());
                }
            }
        });
    }

    // Up directory
    {
        let app_weak = weak.clone();
        let current_path = Arc::clone(&current_path);
        let list_files = Arc::clone(&list_files);

        app.on_up_directory(move || {
            let app = match app_weak.upgrade() {
                Some(a) => a,
                None => return,
            };
            let mut current = current_path.lock().unwrap();
            if let Some(parent) = current.parent() {
                *current = parent.to_path_buf();
                app.set_current_path(SharedString::from(current.to_string_lossy().to_string()));
                drop(current);
                list_files(&app);
            }
        });
    }

    // Go to root
    {
        let app_weak = weak.clone();
        let current_path = Arc::clone(&current_path);
        let list_files = Arc::clone(&list_files);
        let root_path_clone = root_path.clone();

        app.on_go_to_root(move || {
            let app = match app_weak.upgrade() {
                Some(a) => a,
                None => return,
            };
            let mut current = current_path.lock().unwrap();
            *current = PathBuf::from(&root_path_clone);
            app.set_current_path(SharedString::from(root_path_clone.clone()));
            drop(current);
            list_files(&app);
        });
    }

    // Open file
    {
        let weak = weak.clone();
        app.on_open_file(move |path: SharedString| {
            #[cfg(target_os = "windows")]
            {
                let _ = Command::new("cmd")
                    .args(["/C", "start", "", &path])
                    .spawn();
            }
            #[cfg(target_os = "linux")]
            {
                let _ = Command::new("xdg-open")
                    .arg(&path)
                    .spawn();
            }
            #[cfg(target_os = "macos")]
            {
                let _ = Command::new("open")
                    .arg(&path)
                    .spawn();
            }
        });
    }

    // Navigate
    {
        let app_weak = weak.clone();
        let current_path = Arc::clone(&current_path);
        let list_files = Arc::clone(&list_files);

        app.on_navigate(move |path: SharedString| {
            let app = match app_weak.upgrade() {
                Some(a) => a,
                None => return,
            };
            let mut current = current_path.lock().unwrap();
            let new_path = PathBuf::from(path.as_str());
            if new_path.exists() {
                *current = new_path;
                app.set_current_path(SharedString::from(current.to_string_lossy().to_string()));
                drop(current);
                list_files(&app);
            }
        });
    }

    // 4) Initial listing + run
    list_files(&app);
    app.run().unwrap();
}
