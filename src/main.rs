use std::{
    collections::VecDeque,
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

// Slint generates this module name from the code below:
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
// BFS Search
// -----------------------------------------------------------------------------

/// BFS over directories, searching for files whose names match `pattern`.
/// Sends matches to `tx`.
fn bfs_search(root_dir: String, pattern: &regex::Regex, tx: mpsc::Sender<SlintFileEntry>) {
    use std::collections::VecDeque;
    let mut queue = VecDeque::new();
    queue.push_back(PathBuf::from(&root_dir));

    while let Some(current_dir) = queue.pop_front() {
        let rd = match fs::read_dir(&current_dir) {
            Ok(it) => it,
            Err(_) => continue,
        };

        for entry in rd {
            let entry = match entry {
                Ok(e) => e,
                Err(_) => continue,
            };
            let path = entry.path();

            // If it's a directory, enqueue
            if path.is_dir() {
                queue.push_back(path.clone());
            }

            // Check if filename matches
            let filename = path.file_name().unwrap_or_default().to_string_lossy();
            if pattern.is_match(&filename) {
                if let Ok(md) = fs::metadata(&path) {
                    let fe = create_file_entry(&path, &md);
                    let _ = tx.send(fe);
                }
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

    // 1) Show immediate directory listing
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

    // 2) BFS-based search
    let setup_search: Arc<dyn Fn(String) + 'static> = {
        let app_weak = weak.clone();
        let search_state = Arc::clone(&search_state);
        let root_path = root_path.clone();

        Arc::new(move |query: String| {
            let app = match app_weak.upgrade() {
                Some(a) => a,
                None => return,
            };

            // Empty query => no searching
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

            let pattern = RegexBuilder::new(&regex::escape(&query))
                .case_insensitive(true)
                .build()
                .unwrap_or_else(|_| RegexBuilder::new("").build().unwrap());

            // Create the channel
            let (tx, rx) = mpsc::channel();
            let app_weak = app_weak.clone();
            let search_state = Arc::clone(&search_state);

            // Clone root_path for this thread
            let root_path_clone = root_path.clone();

            thread::spawn(move || {
                // BFS search, sending results to tx
                bfs_search(root_path_clone, &pattern, tx);

                // Collect all results
                let mut results = Vec::new();
                // We do NOT call drop(tx) here, so tx is dropped automatically at end of scope
                // That notifies rx that no more items are coming
                while let Ok(fe) = rx.recv() {
                    results.push(fe);
                }

                // Sort results
                results.sort_by(|a, b| {
                    match (a.is_directory, b.is_directory) {
                        (true, false) => std::cmp::Ordering::Less,
                        (false, true) => std::cmp::Ordering::Greater,
                        _ => a.name.as_str().cmp(b.name.as_str()),
                    }
                });

                if let Some(app) = app_weak.upgrade() {
                    let mut state = search_state.lock().unwrap();
                    state.is_searching = false;
                    drop(state);
                    app.set_is_searching(false);

                    // If Slint requires the main thread, do slint::invoke_from_event_loop(...) here.
                    app.set_files(ModelRc::from(Rc::new(VecModel::from(results))));
                }
            });
        })
    };

    // 3) Connect UI callbacks
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
