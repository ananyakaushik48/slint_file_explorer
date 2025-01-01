use std::{fs, path::PathBuf, process::Command, sync::Arc, time::{SystemTime, UNIX_EPOCH}};
use slint::{ModelRc, SharedString, VecModel};
use std::sync::Mutex;

slint::slint! {
    import { Button, VerticalBox, ListView } from "std-widgets.slint";

    export struct FileEntry {
        name: string,
        is_directory: bool,
        path: string,
        size: string,
        modified: string,
    }

    component FileItemView {
        in property <FileEntry> entry;
        callback clicked();
        callback double-clicked();
        
        Rectangle {
            background: ta.pressed ? #2962ff : ta.has-hover ? #90caf9 : transparent;
            ta := TouchArea {
                clicked => { root.clicked(); }
                double-clicked => { root.double-clicked(); }
            }
            HorizontalLayout {
                padding: 8px;
                spacing: 8px;
                Text { 
                    text: root.entry.name;
                    color: ta.has-hover ? black : #f3f3f3;
                }
                Text { 
                    text: root.entry.is_directory ? "[Directory]" : "[File]"; 
                    color: root.entry.is_directory ? #1b5e20 : #f3f3f3;
                }
                Text { 
                    text: root.entry.size;
                    color: ta.has-hover ? black : #f3f3f3;
                }
                Text { 
                    text: root.entry.modified;
                    color: ta.has-hover ? black : #f3f3f3;
                }
            }
        }
    }

    export component MainWindow inherits Window {
        in-out property <[FileEntry]> files: [];
        in-out property <string> current-path: "/";
        callback navigate(string);
        callback open-file(string);
        callback up-directory();
        callback go-to-root();
        
        title: "File Decimator";
        width: 800px;
        height: 600px;

        VerticalLayout {
            padding: 10px;
            spacing: 10px;
            Text { 
                text: root.current-path;
                color: whitesmoke;
            }
            HorizontalLayout {
                spacing: 8px;
                Button { 
                    text: "Up"; 
                    clicked => { root.up-directory(); }
                }
                Button { 
                    text: "Root"; 
                    clicked => { root.go-to-root(); }
                }
            }

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
            "{:04}-{:02}-{:02} {:02}:{:02}:{:02}",
            1970 + (secs / 31536000),
            ((secs % 31536000) / 2592000) + 1,
            ((secs % 2592000) / 86400) + 1,
            (secs % 86400) / 3600,
            (secs % 3600) / 60,
            secs % 60
        )
    } else {
        String::from("Unknown")
    }
}

fn main() {
    let app = MainWindow::new().unwrap();
    let weak = app.as_weak();
    
    #[cfg(target_os = "windows")]
    let root_path = "C:\\";
    #[cfg(not(target_os = "windows"))]
    let root_path = "/";
    
    let current_path = Arc::new(Mutex::new(PathBuf::from(root_path)));

    let current_path_clone = current_path.clone();
    let list_files = Arc::new(move |app: &MainWindow| {
        let path = current_path_clone.lock().unwrap();
        let mut entries = Vec::new();
        
        if let Ok(dir_entries) = fs::read_dir(&*path) {
            for entry in dir_entries {
                if let Ok(entry) = entry {
                    let path = entry.path();
                    // Skip paths that require elevated permissions
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
        app.set_files(ModelRc::new(VecModel::from(entries)));
    });

    let app_weak = weak.clone();
    let current_path_clone3 = current_path.clone();
    let list_files_clone2 = list_files.clone();
    app.on_up_directory(move || {
        let app = app_weak.unwrap();
        let mut current = current_path_clone3.lock().unwrap();
        if let Some(parent) = current.parent() {
            *current = parent.to_path_buf();
            app.set_current_path(SharedString::from(
                current.to_string_lossy().to_string()
            ));
            drop(current);
            list_files_clone2(&app);
        }
    });

    let app_weak = weak.clone();
    let current_path_clone4 = current_path.clone();
    let list_files_clone3 = list_files.clone();
    app.on_go_to_root(move || {
        let app = app_weak.unwrap();
        let mut current = current_path_clone4.lock().unwrap();
        *current = PathBuf::from(root_path);
        app.set_current_path(SharedString::from(root_path));
        drop(current);
        list_files_clone3(&app);
    });

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

    let app_weak = weak.clone();
    let list_files_clone = list_files.clone();
    let current_path_clone2 = current_path.clone();
    app.on_navigate(move |path: SharedString| {
        let app = app_weak.unwrap();
        let mut current = current_path_clone2.lock().unwrap();
        
        let new_path = PathBuf::from(path.as_str());
        if new_path.exists() {
            *current = new_path;
            app.set_current_path(SharedString::from(
                current.to_string_lossy().to_string()
            ));
            drop(current);
            list_files_clone(&app);
        }
    });

    list_files(&app);
    app.run().unwrap();
}