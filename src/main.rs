use std::collections::HashMap;
use std::ffi::OsString;
use std::os::windows::ffi::OsStringExt;
use std::thread;
use std::time::{Duration, Instant};

use windows::{
    core::*,
    Win32::Foundation::*,
    Win32::UI::WindowsAndMessaging::*,
};

// Structure to store window information
#[derive(Debug, Clone, PartialEq)]
struct WindowInfo {
    hwnd: HWND,
    title: String,
    class_name: String,
}

// Cache structure for performance optimization
struct WindowCache {
    windows: Vec<WindowInfo>,
    last_update: Instant,
    cache_duration: Duration,
}

impl WindowCache {
    fn new() -> Self {
        Self {
            windows: Vec::new(),
            last_update: Instant::now() - Duration::from_secs(1), // Force initial update
            cache_duration: Duration::from_millis(50), // Cache for 50ms
        }
    }
    
    fn get_windows(&mut self) -> std::result::Result<&Vec<WindowInfo>, Box<dyn std::error::Error>> {
        if self.last_update.elapsed() > self.cache_duration {
            self.windows = get_all_windows_uncached()?;
            self.last_update = Instant::now();
        }
        Ok(&self.windows)
    }
}

// Callback function for EnumWindows
unsafe extern "system" fn enum_windows_proc(
    hwnd: HWND,
    lparam: LPARAM,
) -> BOOL {
    let windows = unsafe { &mut *(lparam.0 as *mut Vec<WindowInfo>) };
    
    // Only get visible windows that are not child windows
    if unsafe { IsWindowVisible(hwnd).as_bool() && GetParent(hwnd).unwrap_or(HWND(std::ptr::null_mut())) == HWND(std::ptr::null_mut()) } {
        let mut title_buffer = [0u16; 256];
        let mut class_buffer = [0u16; 256];
        
        let title_len = unsafe { GetWindowTextW(hwnd, &mut title_buffer) };
        let class_len = unsafe { GetClassNameW(hwnd, &mut class_buffer) };
        
        if title_len > 0 {
            let title = OsString::from_wide(&title_buffer[..title_len as usize])
                .to_string_lossy()
                .to_string();
            let class_name = OsString::from_wide(&class_buffer[..class_len as usize])
                .to_string_lossy()
                .to_string();
            
            windows.push(WindowInfo {
                hwnd,
                title,
                class_name,
            });
        }
    }
    
    TRUE
}

// Function to get all open windows (uncached)
fn get_all_windows_uncached() -> std::result::Result<Vec<WindowInfo>, Box<dyn std::error::Error>> {
    let mut windows = Vec::with_capacity(50); // Pre-allocate for better performance
    
    unsafe {
        EnumWindows(
            Some(enum_windows_proc),
            LPARAM(&mut windows as *mut _ as isize),
        )?;
    }
    
    Ok(windows)
}

// Function to minimize window
fn minimize_window(hwnd: HWND) -> std::result::Result<(), Box<dyn std::error::Error>> {
    unsafe {
        let _ = ShowWindow(hwnd, SW_MINIMIZE);
    }
    Ok(())
}

// Optimized function to check if window title contains specific keywords
fn is_target_window(window: &WindowInfo, target_keywords: &[String], keyword_cache: &HashMap<String, String>) -> bool {
    let title_lower = window.title.to_lowercase();
    target_keywords.iter().any(|keyword| {
        let keyword_lower = keyword_cache.get(keyword).unwrap();
        title_lower.contains(keyword_lower)
    })
}

// Function to check if window should be skipped (system windows and ignored windows)
fn should_skip_window(window: &WindowInfo, ignored_keywords: &[String], ignored_cache: &HashMap<String, String>) -> bool {
    // Skip empty titles and system windows
    if window.title.is_empty() ||
       window.title.contains("Program Manager") ||
       window.title.contains("Desktop") ||
       window.class_name.contains("Shell_TrayWnd") {
        return true;
    }
    
    // Skip windows that match ignored keywords
    let title_lower = window.title.to_lowercase();
    ignored_keywords.iter().any(|keyword| {
        let keyword_lower = ignored_cache.get(keyword).unwrap();
        title_lower.contains(keyword_lower)
    })
}

// Optimized main function for window monitoring
fn monitor_windows(target_keywords: Vec<String>, ignored_keywords: Vec<String>) -> std::result::Result<(), Box<dyn std::error::Error>> {
    println!("Starting optimized window monitoring...");
    println!("Target keywords: {:?}", target_keywords);
    println!("Ignored keywords: {:?}", ignored_keywords);
    println!("Press Ctrl+C to stop the program\n");
    
    // Pre-compute lowercase keywords for faster comparison
    let keyword_cache: HashMap<String, String> = target_keywords
        .iter()
        .map(|k| (k.clone(), k.to_lowercase()))
        .collect();
    
    let ignored_cache: HashMap<String, String> = ignored_keywords
        .iter()
        .map(|k| (k.clone(), k.to_lowercase()))
        .collect();
    
    let mut last_active_window: Option<HWND> = None;
    let mut window_cache = WindowCache::new();
    
    loop {
        // Get currently active window
        let current_active = unsafe { GetForegroundWindow() };
        
        // Only process if active window changed
        if last_active_window != Some(current_active) {
            last_active_window = Some(current_active);
            
            // Get cached window list
            let windows = window_cache.get_windows()?;
            
            // Find active window in list using early exit
            if let Some(active_window) = windows.iter().find(|w| w.hwnd == current_active) {
                println!("Active window: {}", active_window.title);
                
                // Check if active window is target window
                if is_target_window(active_window, &target_keywords, &keyword_cache) {
                    println!("✓ Target window detected: {}", active_window.title);
                    
                    // Collect windows to minimize (filter first, then minimize)
                    let windows_to_minimize: Vec<&WindowInfo> = windows
                        .iter()
                        .filter(|window| {
                            window.hwnd != current_active &&
                            !is_target_window(window, &target_keywords, &keyword_cache) &&
                            !should_skip_window(window, &ignored_keywords, &ignored_cache)
                        })
                        .collect();
                    
                    // Minimize collected windows
                    let mut minimized_count = 0;
                    for window in windows_to_minimize {
                        if let Err(e) = minimize_window(window.hwnd) {
                            eprintln!("Error minimizing {}: {}", window.title, e);
                        } else {
                            println!("  → Minimized: {}", window.title);
                            minimized_count += 1;
                        }
                    }
                    
                    if minimized_count > 0 {
                        println!("Total {} windows minimized\n", minimized_count);
                    } else {
                        println!("No other windows need to be minimized\n");
                    }
                } else {
                    println!("This window is not a target window\n");
                }
            }
        }
        
        // Reduced wait time for better responsiveness
        thread::sleep(Duration::from_millis(100));
    }
}

fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    println!("Window Monitor for Windows");
    println!("This program will minimize other windows when target windows are opened\n");
    
    // List of keywords for target windows
    // You can modify this according to your needs
    let target_keywords = vec![
        "Trae".to_string(),
        // Add other keywords as needed
    ];
    
    // List of keywords for windows to ignore (never minimize)
    // You can modify this according to your needs
    let ignored_keywords = vec![
        "WhatsApp".to_string(),
        // Add other keywords as needed
    ];
    
    println!("Target windows to monitor:");
    for keyword in &target_keywords {
        println!("  - Windows containing: '{}'", keyword);
    }
    println!();
    
    println!("Windows to ignore (never minimize):");
    for keyword in &ignored_keywords {
        println!("  - Windows containing: '{}'", keyword);
    }
    println!();
    
    // Start monitoring
    monitor_windows(target_keywords, ignored_keywords)?;
    
    Ok(())
}
