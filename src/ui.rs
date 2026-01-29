//! UI module for Process Monitor
//! Contains Dioxus components with Tailwind CSS

use dioxus::prelude::*;
use crate::process::{ProcessInfo, get_processes, get_system_stats, kill_process, open_file_location, format_uptime};

/// Sort column options
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum SortColumn {
    Pid,
    Name,
    Memory,
    Threads,
    Cpu,
}

/// Sort order options
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum SortOrder {
    Ascending,
    Descending,
}

/// Context menu state
#[derive(Clone, Debug, Default)]
pub struct ContextMenuState {
    pub visible: bool,
    pub x: i32,
    pub y: i32,
    pub pid: Option<u32>,
    pub exe_path: String,
}

/// Process row component
#[component]
pub fn ProcessRow(
    process: ProcessInfo,
    is_selected: bool,
    max_memory: f64,
    on_select: EventHandler<u32>,
    on_context_menu: EventHandler<(i32, i32, u32, String)>,
) -> Element {
    let memory_percent = if max_memory > 0.0 { 
        process.memory_mb / max_memory * 100.0 
    } else { 
        0.0 
    };
    let pid = process.pid;
    let exe_path = process.exe_path.clone();
    let exe_path_for_context = process.exe_path.clone();
    let exe_filename = process.exe_path.split('\\').last().unwrap_or(&process.exe_path).to_string();
    
    // CPU usage color based on value
    let cpu_class = if process.cpu_usage > 50.0 {
        "text-red-400"
    } else if process.cpu_usage > 25.0 {
        "text-yellow-400"
    } else {
        "text-green-400"
    };
    
    let row_class = if is_selected {
        "border-l-4 border-red-500 bg-red-500/20 hover:bg-red-500/30 cursor-pointer transition-colors"
    } else {
        "hover:bg-cyan-500/10 cursor-pointer transition-colors border-b border-white/5"
    };

    rsx! {
        tr { 
            key: "{process.pid}",
            class: "{row_class}",
            onclick: move |_| on_select.call(pid),
            oncontextmenu: move |e| {
                e.prevent_default();
                let coords = e.client_coordinates();
                on_context_menu.call((coords.x as i32, coords.y as i32, pid, exe_path_for_context.clone()));
            },
            td { class: "px-4 py-3 font-mono text-yellow-400 w-20", "{process.pid}" }
            td { class: "px-4 py-3 font-medium", "{process.name}" }
            td { class: "px-4 py-3 font-mono {cpu_class} w-20 text-center", "{process.cpu_usage:.1}%" }
            td { class: "px-4 py-3 font-mono text-purple-400 w-20 text-center", "{process.thread_count}" }
            td { class: "px-4 py-3 w-44",
                div { class: "flex items-center gap-2",
                    div { class: "flex-1 h-2 bg-white/10 rounded overflow-hidden",
                        div { 
                            class: "h-full bg-gradient-to-r from-green-400 via-cyan-400 to-red-500 rounded transition-all duration-300",
                            style: "width: {memory_percent}%",
                        }
                    }
                    span { class: "font-mono text-green-400 text-xs min-w-[70px] text-right", "{process.memory_mb:.1} MB" }
                }
            }
            td { class: "px-4 py-3 text-xs text-gray-500 max-w-[200px] truncate hover:text-gray-400", title: "{exe_path}", "{exe_filename}" }
        }
    }
}

/// Main application component
#[component]
pub fn App() -> Element {
    let mut processes = use_signal(|| get_processes());
    let mut system_stats = use_signal(|| get_system_stats());
    let mut search_query = use_signal(|| String::new());
    let mut sort_column = use_signal(|| SortColumn::Memory);
    let mut sort_order = use_signal(|| SortOrder::Descending);
    let mut auto_refresh = use_signal(|| true);
    let mut selected_pid = use_signal(|| None::<u32>);
    let mut status_message = use_signal(|| String::new());
    let mut context_menu = use_signal(|| ContextMenuState::default());

    // Auto-refresh every 3 seconds
    use_future(move || async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(3)).await;
            if *auto_refresh.read() {
                processes.set(get_processes());
                system_stats.set(get_system_stats());
            }
        }
    });

    // Keyboard shortcuts handler
    let handle_keydown = move |e: KeyboardEvent| {
        // Close context menu on Escape
        if e.key() == Key::Escape {
            context_menu.set(ContextMenuState::default());
            return;
        }
        
        // F5 = Refresh
        if e.key() == Key::F5 {
            processes.set(get_processes());
            system_stats.set(get_system_stats());
            return;
        }
        
        // Delete = Kill selected process
        if e.key() == Key::Delete {
            let pid_to_kill = *selected_pid.read();
            if let Some(pid) = pid_to_kill {
                if kill_process(pid) {
                    status_message.set(format!("✓ Process {} terminated", pid));
                    processes.set(get_processes());
                    selected_pid.set(None);
                } else {
                    status_message.set(format!("✗ Failed to terminate process {}", pid));
                }
                spawn(async move {
                    tokio::time::sleep(std::time::Duration::from_secs(3)).await;
                    status_message.set(String::new());
                });
            }
        }
    };

    // Find max memory for percentage calculation
    let max_memory = processes.read().iter().map(|p| p.memory_mb).fold(0.0_f64, |a, b| a.max(b));

    // Filter and sort processes
    let mut filtered_processes: Vec<ProcessInfo> = processes
        .read()
        .iter()
        .filter(|p| {
            let query = search_query.read().to_lowercase();
            if query.is_empty() {
                true
            } else {
                p.name.to_lowercase().contains(&query) 
                    || p.pid.to_string().contains(&query)
                    || p.exe_path.to_lowercase().contains(&query)
            }
        })
        .cloned()
        .collect();

    // Sort based on selected column
    filtered_processes.sort_by(|a, b| {
        let cmp = match *sort_column.read() {
            SortColumn::Pid => a.pid.cmp(&b.pid),
            SortColumn::Name => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
            SortColumn::Memory => a.memory_mb.partial_cmp(&b.memory_mb).unwrap_or(std::cmp::Ordering::Equal),
            SortColumn::Threads => a.thread_count.cmp(&b.thread_count),
            SortColumn::Cpu => a.cpu_usage.partial_cmp(&b.cpu_usage).unwrap_or(std::cmp::Ordering::Equal),
        };
        match *sort_order.read() {
            SortOrder::Ascending => cmp,
            SortOrder::Descending => cmp.reverse(),
        }
    });

    let process_count = filtered_processes.len();
    let total_memory: f64 = filtered_processes.iter().map(|p| p.memory_mb).sum();
    
    // Get system stats
    let stats = system_stats.read().clone();

    // Get current sort state for display
    let current_sort_col = *sort_column.read();
    let current_sort_ord = *sort_order.read();
    
    // Context menu state
    let ctx_menu = context_menu.read().clone();

    // Get sort indicator
    let sort_indicator = |column: SortColumn| -> &'static str {
        if current_sort_col == column {
            match current_sort_ord {
                SortOrder::Ascending => " ▲",
                SortOrder::Descending => " ▼",
            }
        } else {
            ""
        }
    };

    rsx! {
        // Tailwind CDN
        script { src: "https://cdn.tailwindcss.com" }
        style { {CUSTOM_STYLES} }

        // Main container with keyboard handler
        div {
            tabindex: "0",
            onkeydown: handle_keydown,
            onclick: move |_| context_menu.set(ContextMenuState::default()),
            class: "h-screen flex flex-col outline-none",

            // Custom title bar for borderless window
            div { class: "flex justify-between items-center h-9 bg-gradient-to-r from-slate-950 to-slate-900 border-b border-cyan-500/20 select-none flex-shrink-0",
                div { 
                    class: "flex-1 h-full flex items-center pl-3 cursor-move",
                    onmousedown: move |_| {
                        let window = dioxus::desktop::window();
                        let _ = window.drag_window();
                    },
                    span { class: "text-sm font-medium text-cyan-400", "🖥️ Process Monitor" }
                }
                div { class: "flex h-full",
                    button {
                        class: "w-12 h-full border-none bg-transparent text-gray-400 text-xs cursor-pointer transition-all hover:bg-white/10 hover:text-white",
                        onclick: move |_| {
                            let window = dioxus::desktop::window();
                            window.set_minimized(true);
                        },
                        "─"
                    }
                    button {
                        class: "w-12 h-full border-none bg-transparent text-gray-400 text-xs cursor-pointer transition-all hover:bg-white/10 hover:text-white",
                        onclick: move |_| {
                            let window = dioxus::desktop::window();
                            window.set_maximized(!window.is_maximized());
                        },
                        "□"
                    }
                    button {
                        class: "w-12 h-full border-none bg-transparent text-gray-400 text-xs cursor-pointer transition-all hover:bg-red-600 hover:text-white",
                        onclick: move |_| {
                            let window = dioxus::desktop::window();
                            window.close();
                        },
                        "✕"
                    }
                }
            }

            // System Stats Bar
            div { class: "bg-gradient-to-r from-slate-900/80 to-slate-800/80 border-b border-cyan-500/10 px-5 py-2 flex items-center gap-6 text-xs flex-shrink-0",
                // CPU Usage
                div { class: "flex items-center gap-2",
                    span { class: "text-gray-500", "CPU" }
                    div { class: "w-24 h-2 bg-white/10 rounded overflow-hidden",
                        div { 
                            class: "h-full bg-gradient-to-r from-cyan-400 to-cyan-600 transition-all duration-500",
                            style: "width: {stats.cpu_usage}%",
                        }
                    }
                    span { class: "font-mono text-cyan-400 min-w-[40px]", "{stats.cpu_usage:.1}%" }
                }
                
                // Memory Usage
                div { class: "flex items-center gap-2",
                    span { class: "text-gray-500", "RAM" }
                    div { class: "w-24 h-2 bg-white/10 rounded overflow-hidden",
                        div { 
                            class: "h-full bg-gradient-to-r from-purple-400 to-purple-600 transition-all duration-500",
                            style: "width: {stats.memory_percent}%",
                        }
                    }
                    span { class: "font-mono text-purple-400 min-w-[100px]", "{stats.used_memory_gb:.1}/{stats.total_memory_gb:.1} GB" }
                }
                
                // Uptime
                div { class: "flex items-center gap-2",
                    span { class: "text-gray-500", "Uptime" }
                    span { class: "font-mono text-green-400", "{format_uptime(stats.uptime_seconds)}" }
                }
                
                // Process count
                div { class: "flex items-center gap-2 ml-auto",
                    span { class: "text-gray-500", "Total Processes" }
                    span { class: "font-mono text-yellow-400", "{stats.process_count}" }
                }
            }

            div { class: "max-w-6xl mx-auto p-5 flex-1 overflow-hidden flex flex-col",
                // Header
                div { class: "text-center mb-4 p-4 bg-white/5 rounded-xl backdrop-blur-sm flex-shrink-0",
                    h1 { class: "text-2xl mb-2 text-cyan-400 font-bold", "🖥️ Windows Process Monitor" }
                    div { class: "flex justify-center gap-8 text-sm text-gray-400",
                        span { "Showing: {process_count} processes" }
                        span { "Memory: {total_memory:.1} MB" }
                        span { class: "text-gray-600 text-xs", "F5: Refresh | Del: Kill | Esc: Close menu" }
                    }
                    if !status_message.read().is_empty() {
                        div { class: "mt-3 py-2 px-4 bg-cyan-500/20 rounded-md text-sm text-cyan-400 inline-block", "{status_message}" }
                    }
                }

                // Controls
                div { class: "flex gap-4 mb-4 items-center flex-wrap flex-shrink-0",
                    input {
                        class: "flex-1 min-w-[200px] py-3 px-4 border-none rounded-lg bg-white/10 text-white text-sm outline-none transition-colors focus:bg-white/15 placeholder:text-gray-500",
                        r#type: "text",
                        placeholder: "Search by name, PID, or path... (Ctrl+F)",
                        value: "{search_query}",
                        oninput: move |e| search_query.set(e.value().clone()),
                    }
                    
                    label { class: "flex items-center gap-2 text-gray-400 text-sm cursor-pointer select-none",
                        input {
                            r#type: "checkbox",
                            class: "w-4 h-4 cursor-pointer accent-cyan-400",
                            checked: *auto_refresh.read(),
                            onchange: move |e| auto_refresh.set(e.checked()),
                        }
                        span { "Auto-refresh" }
                    }

                    button {
                        class: "py-3 px-6 border-none rounded-lg text-sm font-semibold cursor-pointer transition-all bg-gradient-to-br from-cyan-400 to-cyan-600 text-white hover:-translate-y-0.5 hover:shadow-lg hover:shadow-cyan-500/40 active:translate-y-0",
                        onclick: move |_| {
                            processes.set(get_processes());
                            system_stats.set(get_system_stats());
                        },
                        "🔄 Refresh"
                    }

                    button {
                        class: "py-3 px-6 border-none rounded-lg text-sm font-semibold cursor-pointer transition-all bg-gradient-to-br from-red-500 to-red-700 text-white hover:-translate-y-0.5 hover:shadow-lg hover:shadow-red-500/40 active:translate-y-0 disabled:opacity-50 disabled:cursor-not-allowed disabled:hover:translate-y-0 disabled:hover:shadow-none",
                        disabled: selected_pid.read().is_none(),
                        onclick: move |_| {
                            let pid_to_kill = *selected_pid.read();
                            if let Some(pid) = pid_to_kill {
                                if kill_process(pid) {
                                    status_message.set(format!("✓ Process {} terminated", pid));
                                    processes.set(get_processes());
                                    selected_pid.set(None);
                                } else {
                                    status_message.set(format!("✗ Failed to terminate process {} (access denied?)", pid));
                                }
                                spawn(async move {
                                    tokio::time::sleep(std::time::Duration::from_secs(3)).await;
                                    status_message.set(String::new());
                                });
                            }
                        },
                        "☠️ Kill Process"
                    }
                }

                // Process table
                div { class: "bg-white/5 rounded-xl flex-1 overflow-y-auto overflow-x-hidden min-h-0",
                    table { class: "w-full border-collapse",
                        thead { class: "sticky top-0 bg-cyan-500/20 backdrop-blur-sm z-10",
                            tr {
                                th { 
                                    class: "px-4 py-3 text-left font-semibold text-cyan-400 border-b-2 border-cyan-500/30 cursor-pointer select-none transition-colors hover:bg-cyan-500/30 text-sm",
                                    onclick: move |_| {
                                        if *sort_column.read() == SortColumn::Pid {
                                            let new_order = if *sort_order.read() == SortOrder::Ascending { SortOrder::Descending } else { SortOrder::Ascending };
                                            sort_order.set(new_order);
                                        } else {
                                            sort_column.set(SortColumn::Pid);
                                            sort_order.set(SortOrder::Descending);
                                        }
                                    },
                                    "PID{sort_indicator(SortColumn::Pid)}" 
                                }
                                th { 
                                    class: "px-4 py-3 text-left font-semibold text-cyan-400 border-b-2 border-cyan-500/30 cursor-pointer select-none transition-colors hover:bg-cyan-500/30 text-sm",
                                    onclick: move |_| {
                                        if *sort_column.read() == SortColumn::Name {
                                            let new_order = if *sort_order.read() == SortOrder::Ascending { SortOrder::Descending } else { SortOrder::Ascending };
                                            sort_order.set(new_order);
                                        } else {
                                            sort_column.set(SortColumn::Name);
                                            sort_order.set(SortOrder::Descending);
                                        }
                                    },
                                    "Name{sort_indicator(SortColumn::Name)}" 
                                }
                                th { 
                                    class: "px-4 py-3 text-left font-semibold text-cyan-400 border-b-2 border-cyan-500/30 cursor-pointer select-none transition-colors hover:bg-cyan-500/30 text-sm",
                                    onclick: move |_| {
                                        if *sort_column.read() == SortColumn::Cpu {
                                            let new_order = if *sort_order.read() == SortOrder::Ascending { SortOrder::Descending } else { SortOrder::Ascending };
                                            sort_order.set(new_order);
                                        } else {
                                            sort_column.set(SortColumn::Cpu);
                                            sort_order.set(SortOrder::Descending);
                                        }
                                    },
                                    "CPU{sort_indicator(SortColumn::Cpu)}" 
                                }
                                th { 
                                    class: "px-4 py-3 text-left font-semibold text-cyan-400 border-b-2 border-cyan-500/30 cursor-pointer select-none transition-colors hover:bg-cyan-500/30 text-sm",
                                    onclick: move |_| {
                                        if *sort_column.read() == SortColumn::Threads {
                                            let new_order = if *sort_order.read() == SortOrder::Ascending { SortOrder::Descending } else { SortOrder::Ascending };
                                            sort_order.set(new_order);
                                        } else {
                                            sort_column.set(SortColumn::Threads);
                                            sort_order.set(SortOrder::Descending);
                                        }
                                    },
                                    "Threads{sort_indicator(SortColumn::Threads)}" 
                                }
                                th { 
                                    class: "px-4 py-3 text-left font-semibold text-cyan-400 border-b-2 border-cyan-500/30 cursor-pointer select-none transition-colors hover:bg-cyan-500/30 text-sm",
                                    onclick: move |_| {
                                        if *sort_column.read() == SortColumn::Memory {
                                            let new_order = if *sort_order.read() == SortOrder::Ascending { SortOrder::Descending } else { SortOrder::Ascending };
                                            sort_order.set(new_order);
                                        } else {
                                            sort_column.set(SortColumn::Memory);
                                            sort_order.set(SortOrder::Descending);
                                        }
                                    },
                                    "Memory{sort_indicator(SortColumn::Memory)}" 
                                }
                                th { class: "px-4 py-3 text-left font-semibold text-cyan-400 border-b-2 border-cyan-500/30 text-sm", "Path" }
                            }
                        }
                        tbody {
                            for process in filtered_processes {
                                ProcessRow { 
                                    process: process.clone(),
                                    is_selected: *selected_pid.read() == Some(process.pid),
                                    max_memory: max_memory,
                                    on_select: move |pid: u32| {
                                        let current = *selected_pid.read();
                                        if current == Some(pid) {
                                            selected_pid.set(None);
                                        } else {
                                            selected_pid.set(Some(pid));
                                        }
                                    },
                                    on_context_menu: move |(x, y, pid, path): (i32, i32, u32, String)| {
                                        selected_pid.set(Some(pid));
                                        context_menu.set(ContextMenuState {
                                            visible: true,
                                            x,
                                            y,
                                            pid: Some(pid),
                                            exe_path: path,
                                        });
                                    },
                                }
                            }
                        }
                    }
                }
            }

            // Context Menu
            if ctx_menu.visible {
                div {
                    class: "fixed bg-slate-800 border border-cyan-500/30 rounded-lg shadow-2xl shadow-black/50 py-1 min-w-[180px] z-50",
                    style: "left: {ctx_menu.x}px; top: {ctx_menu.y}px;",
                    onclick: move |e| e.stop_propagation(),
                    
                    // Kill Process
                    button {
                        class: "w-full px-4 py-2 text-left text-sm text-red-400 hover:bg-red-500/20 flex items-center gap-2 transition-colors",
                        onclick: move |_| {
                            if let Some(pid) = ctx_menu.pid {
                                if kill_process(pid) {
                                    status_message.set(format!("✓ Process {} terminated", pid));
                                    processes.set(get_processes());
                                    selected_pid.set(None);
                                } else {
                                    status_message.set(format!("✗ Failed to terminate process {}", pid));
                                }
                                spawn(async move {
                                    tokio::time::sleep(std::time::Duration::from_secs(3)).await;
                                    status_message.set(String::new());
                                });
                            }
                            context_menu.set(ContextMenuState::default());
                        },
                        span { "☠️" }
                        span { "Kill Process" }
                    }
                    
                    // Separator
                    div { class: "h-px bg-cyan-500/20 my-1" }
                    
                    // Open File Location
                    button {
                        class: "w-full px-4 py-2 text-left text-sm text-gray-300 hover:bg-cyan-500/20 flex items-center gap-2 transition-colors disabled:opacity-50 disabled:cursor-not-allowed",
                        disabled: ctx_menu.exe_path.is_empty(),
                        onclick: {
                            let path = ctx_menu.exe_path.clone();
                            move |_| {
                                open_file_location(&path);
                                context_menu.set(ContextMenuState::default());
                            }
                        },
                        span { "📂" }
                        span { "Open File Location" }
                    }
                    
                    // Copy PID
                    button {
                        class: "w-full px-4 py-2 text-left text-sm text-gray-300 hover:bg-cyan-500/20 flex items-center gap-2 transition-colors",
                        onclick: move |_| {
                            if let Some(pid) = ctx_menu.pid {
                                let eval = document::eval(&format!(
                                    r#"navigator.clipboard.writeText("{}")"#,
                                    pid
                                ));
                                let _ = eval;
                                status_message.set(format!("📋 PID {} copied to clipboard", pid));
                                spawn(async move {
                                    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                                    status_message.set(String::new());
                                });
                            }
                            context_menu.set(ContextMenuState::default());
                        },
                        span { "📋" }
                        span { "Copy PID" }
                    }
                    
                    // Copy Path
                    button {
                        class: "w-full px-4 py-2 text-left text-sm text-gray-300 hover:bg-cyan-500/20 flex items-center gap-2 transition-colors disabled:opacity-50 disabled:cursor-not-allowed",
                        disabled: ctx_menu.exe_path.is_empty(),
                        onclick: {
                            let path = ctx_menu.exe_path.clone();
                            move |_| {
                                let eval = document::eval(&format!(
                                    r#"navigator.clipboard.writeText("{}")"#,
                                    path.replace('\\', "\\\\")
                                ));
                                let _ = eval;
                                status_message.set("📋 Path copied to clipboard".to_string());
                                spawn(async move {
                                    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                                    status_message.set(String::new());
                                });
                                context_menu.set(ContextMenuState::default());
                            }
                        },
                        span { "📝" }
                        span { "Copy Path" }
                    }
                    
                    // Separator
                    div { class: "h-px bg-cyan-500/20 my-1" }
                    
                    // Refresh
                    button {
                        class: "w-full px-4 py-2 text-left text-sm text-gray-300 hover:bg-cyan-500/20 flex items-center gap-2 transition-colors",
                        onclick: move |_| {
                            processes.set(get_processes());
                            system_stats.set(get_system_stats());
                            context_menu.set(ContextMenuState::default());
                        },
                        span { "🔄" }
                        span { "Refresh List" }
                    }
                }
            }
        }
    }
}

/// Minimal custom styles (only for things Tailwind can't handle easily)
pub const CUSTOM_STYLES: &str = r#"
    * {
        margin: 0;
        padding: 0;
        box-sizing: border-box;
    }

    html, body {
        font-family: 'Segoe UI', Tahoma, Geneva, Verdana, sans-serif;
        background: linear-gradient(135deg, #1a1a2e 0%, #16213e 100%);
        color: #eee;
        height: 100%;
        overflow: hidden;
    }

    ::-webkit-scrollbar {
        width: 6px;
        height: 6px;
    }

    ::-webkit-scrollbar-track {
        background: transparent;
    }

    ::-webkit-scrollbar-thumb {
        background: rgba(0, 212, 255, 0.3);
        border-radius: 3px;
    }

    ::-webkit-scrollbar-thumb:hover {
        background: rgba(0, 212, 255, 0.5);
    }
"#;
