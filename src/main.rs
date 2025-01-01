use std::{fs, path::PathBuf, process::Command, sync::Arc, time::{SystemTime, UNIX_EPOCH}};
use slint::{ModelRc, SharedString, VecModel};
use std::sync::Mutex;
use walkdir::WalkDir;
use std::thread;

slint::slint! {
    import { Button, VerticalBox, ListView } from "std-widgets.slint";

    struct Theme {
        primary: brush,
        secondary: brush,
        text: brush,
        accent: brush,
        hover: brush,
        background: brush,
    }

    global Palette {
        out property <Theme> theme: {
            primary: #1a1a1a,
            secondary: #2d2d2d,
            text: #ffffff,
            accent: #007acc,
            hover: #3d3d3d,
            background: #000000,
        };
    }

    export struct FileEntry {
        name: string,
        is_directory: bool,
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
            background: ta.pressed ? Palette.theme.accent : 
                       ta.has-hover ? Palette.theme.hover : transparent;
            border-radius: 4px;

            ta := TouchArea {
                clicked => { root.clicked(); }
                double-clicked => { root.double-clicked(); }
            }

            HorizontalLayout {
                padding: 12px;
                spacing: 12px;

                // Icon representation using text
                Text {
                    text: root.entry.is_directory ? "📁" : "📄";
                    font-size: 16px;
                }

                Text { 
                    text: root.entry.name;
                    color: Palette.theme.text;
                    font-size: 14px;
                }

                Text { 
                    text: root.entry.size;
                    color: Palette.theme.text;
                    font-size: 14px;
                }

                Text { 
                    text: root.entry.modified;
                    color: Palette.theme.text;
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
                        text: "Up";
                        clicked => { root.up-directory(); }
                    }
                }

                VerticalBox {
                    Button { 
                        text: "Root";
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
                    text: "Searching...";
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
        format!(
            "{:04}-{:02}-{:02} {:02}:{:02}",
            1970 + (secs / 31536000),
            ((secs % 31536000) / 2592000) + 1,
            ((secs % 2592000) / 86400) + 1,
            (secs % 86400) / 3600,
            (secs % 3600) / 60
        )
    } else {
        String::from("Unknown")
    }
}

struct SearchState {
    query: String,
    results: Vec<slint::SharedString>,
    is_searching: bool,
}

fn main() {
    let app = MainWindow::new().unwrap();
    let weak = app.as_weak();
    
    #[cfg(target_os = "windows")]
    let root_path = "C:\\";
    #[cfg(not(target_os = "windows"))]
    let root_path = "/";
    
    let current_path = Arc::new(Mutex::new(PathBuf::from(root_path)));
    let search_state = Arc::new(Mutex::new(SearchState {
        query: String::new(),
        results: Vec::new(),
        is_searching: false,
    }));

    // Create list_files function with explicit type annotation
    let list_files: Arc<dyn Fn(&MainWindow) + Send + Sync> = Arc::new({
        let current_path = current_path.clone();
        move |app: &MainWindow| {
            let path = current_path.lock().unwrap();
            let mut entries = Vec::new();
            
            if let Ok(dir_entries) = fs::read_dir(&*path) {
                for entry in dir_entries {
                    if let Ok(entry) = entry {
                        let path = entry.path();
                        if let Ok(metadata) = fs::metadata(&path) {
                            entries.push(FileEntry {
                                name: SharedString::from(path.file_name()
                                    .unwrap_or_default()
                                    .to_string_lossy()
                                    .to_string()),
                                is_directory: path.is_dir(),
                                path: SharedString::from(path.to_string_lossy().to_string()),
                                size: SharedString::from(format_size(metadata.len())),
                                modified: SharedString::from(format_time(
                                    metadata.modified().unwrap_or(SystemTime::now())
                                )),
                            });
                        }
                    }
                }
            }

            entries.sort_by(|a, b| {
                if a.is_directory == b.is_directory {
                    a.name.cmp(&b.name)
                } else {
                    b.is_directory.cmp(&a.is_directory)
                }
            });

            app.set_files(ModelRc::new(VecModel::from(entries)));
        }
    });

    // Setup UI callbacks with proper cloning
    {
        let app_weak = weak.clone();
        let search_state = search_state.clone();
        let list_files = list_files.clone();
        app.on_search_text_changed(move |text: SharedString| {
            let app = app_weak.unwrap();
            let mut state = search_state.lock().unwrap();
            state.query = text.to_string();
            
            if state.query.is_empty() {
                state.is_searching = false;
                app.set_is_searching(false);
                drop(state);
                list_files(&app);
            }
        });
    }

    {
        let app_weak = weak.clone();
        let search_state = search_state.clone();
        app.on_search_submitted(move || {
            let app = app_weak.unwrap();
            let mut state = search_state.lock().unwrap();
            
            if state.query.is_empty() {
                return;
            }

            state.is_searching = true;
            app.set_is_searching(true);
            let query = state.query.clone();
            drop(state);

            // Spawn search thread
            let search_state = search_state.clone();
            let app_weak = app_weak.clone();
            thread::spawn(move || {
                let mut results = Vec::new();
                
                for entry in WalkDir::new(root_path)
                    .follow_links(true)
                    .into_iter()
                    .filter_map(|e| e.ok())
                {
                    let path = entry.path();
                    if let Ok(metadata) = fs::metadata(path) {
                        let name = path.file_name()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .to_string();
                        
                        if name.to_lowercase().contains(&query.to_lowercase()) {
                            results.push(FileEntry {
                                name: SharedString::from(name),
                                is_directory: path.is_dir(),
                                path: SharedString::from(path.to_string_lossy().to_string()),
                                size: SharedString::from(format_size(metadata.len())),
                                modified: SharedString::from(format_time(
                                    metadata.modified().unwrap_or(SystemTime::now())
                                )),
                            });
                        }
                    }
                }

                if let Some(app) = app_weak.upgrade() {
                    let mut state = search_state.lock().unwrap();
                    state.is_searching = false;
                    app.set_is_searching(false);
                    app.set_files(ModelRc::new(VecModel::from(results)));
                }
            });
        });
    }

    {
        let app_weak = weak.clone();
        let current_path = current_path.clone();
        let list_files = list_files.clone();
        app.on_up_directory(move || {
            let app = app_weak.unwrap();
            let mut current = current_path.lock().unwrap();
            if let Some(parent) = current.parent() {
                *current = parent.to_path_buf();
                app.set_current_path(SharedString::from(
                    current.to_string_lossy().to_string()
                ));
                drop(current);
                list_files(&app);
            }
        });
    }

    {
        let app_weak = weak.clone();
        let current_path = current_path.clone();
        let list_files = list_files.clone();
        app.on_go_to_root(move || {
            let app = app_weak.unwrap();
            let mut current = current_path.lock().unwrap();
            *current = PathBuf::from(root_path);
            app.set_current_path(SharedString::from(root_path));
            drop(current);
            list_files(&app);
        });
    }

    app.on_open_file(move |path: SharedString| {
        #[cfg(target_os = "windows")]
        Command::new("cmd")
            .args(["/C", "start", "", &path])
            .spawn()
            .ok();

        #[cfg(target_os = "linux")]
        Command::new("xdg-open")
            .arg(&path)
            .spawn()
            .ok();

        #[cfg(target_os = "macos")]
        Command::new("open")
            .arg(&path)
            .spawn()
            .ok();
    });

    {
        let app_weak = weak.clone();
        let current_path = current_path.clone();
        let list_files = list_files.clone();
        app.on_navigate(move |path: SharedString| {
            let app = app_weak.unwrap();
            let mut current = current_path.lock().unwrap();
            
            let new_path = PathBuf::from(path.as_str());
            if new_path.exists() {
                *current = new_path;
                app.set_current_path(SharedString::from(
                    current.to_string_lossy().to_string()
                ));
                drop(current);
                list_files(&app);
            }
        });
    }

    // Initial file listing
    list_files(&app);
    app.run().unwrap();
}