use io::ErrorKind;
use serde::Deserialize;
use std::io::{self, BufWriter, Error, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::Duration;
use std::{env, ptr};
use std::{fs, mem::size_of};
use tokio::task;
use tokio::time::sleep;
use std::mem;
use windows_sys::Win32::Foundation::HANDLE;
use windows_sys::Win32::Security::{GetTokenInformation, TokenElevation, TOKEN_ELEVATION, TOKEN_QUERY};
use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Once;
use indicatif;

#[derive(Debug, Deserialize)]
struct BenchmarkingParameters {
    #[serde(rename = "StartValue")]
    start_value: f64,
    #[serde(rename = "IncrementValue")]
    increment_value: f64,
    #[serde(rename = "EndValue")]
    end_value: f64,
    #[serde(rename = "SampleValue")]
    sample_value: i32,
}

#[tokio::main]
async fn main() -> io::Result<()> {
    // Create a dynamic separator using '=' characters
    let separator = "=".repeat(60);
    
    // Title Block
    println!("\n{}", separator);
    println!("{:^60}", "🚀 Timer Resolution Benchmark Tool");
    println!("{}\n", separator);

    // Check admin privileges first - fail fast
    if !is_admin() {
        eprintln!("❌ Error: Administrator privileges required!");
        eprintln!("   Please run this program as Administrator.");
        return Err(Error::new(ErrorKind::PermissionDenied, "Administrator privileges required"));
    }

    // System information block
    println!("📊 System Information");
    println!("━━━━━━━━━━━━━━━━━━━");
    println!("📂 Working Directory: {}", env::current_dir()?.display());
    println!("🛡️ Admin Privileges: ✓ Confirmed\n");

    // HPET Configuration block
    println!("🔧 System Configuration");
    println!("━━━━━━━━━━━━━━━━━━━━");
    check_hpet_status()?;
    println!();

    // Load and parse configuration
    let parameters = match fs::read_to_string("appsettings.json")
        .and_then(|content| serde_json::from_str::<BenchmarkingParameters>(&content)
            .map_err(|e| Error::new(ErrorKind::InvalidData, e)))
    {
        Ok(mut params) => {
            let mut input = String::new();

            let mut prompt = |desc: &str, current: &str| -> io::Result<String> {
                println!("▸ {}: {} (current)", desc, current);
                println!("Enter new {} (or press Enter to keep current): ", desc);
                input.clear();
                io::stdin().read_line(&mut input)?;
                Ok(input.trim().to_string())
            };

            println!("⚙️ Benchmark Parameters");
            println!("━━━━━━━━━━━━━━━━━━━");

            if let Ok(new_value) = prompt("Start Value", &format!("{:.4} ms", params.start_value)) {
                if !new_value.is_empty() {
                    params.start_value = new_value.parse().map_err(|e| Error::new(ErrorKind::InvalidInput, e))?;
                }
            }

            if let Ok(new_value) = prompt("Increment Value", &format!("{:.4} ms", params.increment_value)) {
                if !new_value.is_empty() {
                    params.increment_value = new_value.parse().map_err(|e| Error::new(ErrorKind::InvalidInput, e))?;
                }
            }

            if let Ok(new_value) = prompt("End Value", &format!("{:.4} ms", params.end_value)) {
                if !new_value.is_empty() {
                    params.end_value = new_value.parse().map_err(|e| Error::new(ErrorKind::InvalidInput, e))?;
                }
            }

            if let Ok(new_value) = prompt("Sample Value", &params.sample_value.to_string()) {
                if !new_value.is_empty() {
                    params.sample_value = new_value.parse().map_err(|e| Error::new(ErrorKind::InvalidInput, e))?;
                }
            }

            let iterations = ((params.end_value - params.start_value) / params.increment_value).ceil();
            println!("▸ Iterations:   {}\n", iterations as i32);

            params
        },
        Err(e) => {
            eprintln!("❌ Configuration Error: {}", e);
            return Err(e);
        }
    };

    let exe_dir = match env::current_exe() {
        Ok(path) => path.parent().map_or_else(|| PathBuf::from("."), |p| p.to_path_buf()),
        Err(e) => {
            eprintln!("❌ Error: Failed to get current executable path: {}", e);
            return Err(Error::new(ErrorKind::Other, "Failed to get current executable path"));
        }
    };

    let set_timer_resolution_path = exe_dir.join("SetTimerResolution.exe");
    let measure_sleep_path = exe_dir.join("MeasureSleep.exe");

    // Dependency check
    println!("\n🔍 Checking Dependencies");
    println!("━━━━━━━━━━━━━━━━━━━━━");
    for dep in &[&set_timer_resolution_path, &measure_sleep_path] {
        if !dep.exists() {
            eprintln!("❌ Error: Missing dependency: {}", dep.display());
            return Err(Error::new(ErrorKind::NotFound, format!("Dependency not found: {}", dep.display())));
        }
        println!("✓ Found: {}", dep.file_name().unwrap_or_default().to_string_lossy());
    }
    println!();

    println!("\n⏳ Press Enter to start the benchmark...");
    let mut input = String::new();
    io::stdin().read_line(&mut input)?;

    let mut results_file = BufWriter::new(fs::File::create("results.txt")?);
    writeln!(results_file, "RequestedResolutionMs,DeltaMs,StandardDeviation")?;
    println!("📝 Results will be saved to: results.txt");

    let mut current = parameters.start_value;
    let total_iterations = ((parameters.end_value - parameters.start_value) / parameters.increment_value).ceil() as u64;
    let progress_bar = indicatif::ProgressBar::new(total_iterations);
    progress_bar.set_style(
        indicatif::ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({eta}) {msg}")
            .unwrap()
            .progress_chars("#>-")
    );
    progress_bar.enable_steady_tick(Duration::from_millis(100));

    while current <= parameters.end_value {
        let resolution = ((current * 10_000.0).round() / 10_000.0 * 10_000.0) as i32;
    
        let timer_path = set_timer_resolution_path.clone();
        let sleep_path = measure_sleep_path.clone();
    
        let set_timer_result = task::spawn_blocking(move || {
            Command::new(&timer_path)
                .args(&["--resolution", &resolution.to_string(), "--no-console"])
                .stdout(Stdio::null())
                .spawn()
        }).await?;

        match set_timer_result {
            Ok(_) => {},
            Err(e) => eprintln!("Failed to set timer resolution: {}", e),
        }
    
        sleep(Duration::from_millis(1)).await;
    
        let output = match Command::new(&sleep_path)
            .arg("--samples")
            .arg(parameters.sample_value.to_string())
            .output() {
            Ok(out) => out,
            Err(e) => {
                eprintln!("Failed to execute measurement: {}", e);
                return Err(e);
            }
        };
    
        let (avg, stdev) = match parse_measurement_output(&output.stdout) {
            Ok(result) => result,
            Err(e) => {
                eprintln!("Failed to parse measurement output: {}", e);
                return Err(e);
            }
        };
    
        if avg != 0.0 && stdev != 0.0 {
            writeln!(results_file, "{:.4},{:.4},{:.4}", current, avg, stdev)?;
        } else {
            eprintln!("❌ Measurement output is invalid for resolution: {}", resolution);
        }
    
        results_file.flush()?;
        kill_process("SetTimerResolution.exe");
    
        current += parameters.increment_value;
        progress_bar.set_message(format!("Current resolution: {:.4} ms", current));
        progress_bar.inc(1);
    }

    progress_bar.finish_with_message("Benchmarking completed successfully");

    // Confirm the existence of the results file
    let results_path = PathBuf::from("results.txt");
    if !results_path.exists() {
        eprintln!("❌ Error: results.txt file not found!");
        return Err(Error::new(ErrorKind::NotFound, "results.txt file not found"));
    }

    // Read and process the results file
    let results_content = fs::read_to_string(&results_path)?;
    let mut rdr = csv::ReaderBuilder::new()
        .has_headers(true)
        .from_reader(results_content.as_bytes());

    let mut optimal_resolution = None;
    let mut min_delta_ms = f64::MAX;
    let mut min_std_dev = f64::MAX;

    for result in rdr.records() {
        let record = result?;
        let requested_resolution: f64 = record[0].parse().map_err(|e| Error::new(ErrorKind::InvalidData, e))?;
        let delta_ms: f64 = record[1].parse().map_err(|e| Error::new(ErrorKind::InvalidData, e))?;
        let std_dev: f64 = record[2].parse().map_err(|e| Error::new(ErrorKind::InvalidData, e))?;

        if delta_ms < min_delta_ms || (delta_ms == min_delta_ms && std_dev < min_std_dev) {
            min_delta_ms = delta_ms;
            min_std_dev = std_dev;
            optimal_resolution = Some(requested_resolution);
        }
    }

    if let Some(resolution) = optimal_resolution {
        println!("✅ Optimal Timer Resolution: {:.4} ms", resolution);
    } else {
        eprintln!("❌ Error: No valid data found in results.txt");
        return Err(Error::new(ErrorKind::InvalidData, "No valid data found in results.txt"));
    }

    println!("Benchmarking completed successfully");

    // Wait for user input before exiting
    println!("Press Enter to exit...");
    let mut exit_input = String::new();
    io::stdin().read_line(&mut exit_input)?;

    Ok(())
    }

fn check_hpet_status() -> io::Result<()> {
    let output = Command::new("bcdedit")
        .args(&["/enum", "{current}"])
        .output()?;

    if !output.status.success() {
        eprintln!("❌ Error: Failed to retrieve HPET status");
        return Err(Error::new(ErrorKind::Other, "Failed to retrieve HPET status"));
    }

    let output_str = String::from_utf8_lossy(&output.stdout);
    let mut useplatformtick = None;
    let mut disabledynamictick = None;

    for line in output_str.lines() {
        if let Some(value) = line.split_whitespace().nth(1) {
            match line.split_whitespace().next() {
                Some("useplatformtick") => useplatformtick = Some(value.to_lowercase()),
                Some("disabledynamictick") => disabledynamictick = Some(value.to_lowercase()),
                _ => {}
            }
        }
    }

    let hpet_status = match (useplatformtick.as_deref(), disabledynamictick.as_deref()) {
        (Some("no"), Some("yes")) => "disabled",
        _ => "enabled",
    };

    println!("HPET status: {}", hpet_status);

    if hpet_status == "enabled" {
        println!("⚠️ HPET is enabled. For optimal results, it is recommended to disable HPET.");
        println!("Please refer to the troubleshooting guide: https://github.com/SwiftyPop/TimerResBenchmark?tab=readme-ov-file#troubleshooting");
    }

    Ok(())
}

fn parse_measurement_output(output: &[u8]) -> io::Result<(f64, f64)> {
    let output_str = std::str::from_utf8(output)
        .map_err(|e| Error::new(ErrorKind::InvalidData, e))?;

    let lines: Vec<&str> = output_str.lines().collect();
    if lines.len() >= 2 {
        println!("Measurement output: {}", output_str);
    

    let mut avg = 0.0;
    let mut stdev = 0.0;

    for line in output_str.lines() {
        if line.starts_with("Avg: ") {
            avg = line[5..].parse().unwrap_or(0.0);
        } else if line.starts_with("STDEV: ") {
            stdev = line[7..].parse().unwrap_or(0.0);
        }
    }

    Ok((avg, stdev))
    } else {
        Err(Error::new(ErrorKind::InvalidData, "Invalid measurement output"))
    }
}

static IS_ADMIN: AtomicBool = AtomicBool::new(false);
static INIT: Once = Once::new();

fn is_admin() -> bool {
    INIT.call_once(|| {
        unsafe {
            let mut token: HANDLE = ptr::null_mut();
            let is_elevated = if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) != 0 {
                let mut elevation: TOKEN_ELEVATION = mem::zeroed();
                let mut size = size_of::<TOKEN_ELEVATION>() as u32;

                let result = GetTokenInformation(
                    token,
                    TokenElevation,
                    &mut elevation as *mut _ as *mut std::ffi::c_void,
                    size,
                    &mut size,
                );

                windows_sys::Win32::Foundation::CloseHandle(token);
                result != 0 && elevation.TokenIsElevated != 0
            } else {
                false
            };

            IS_ADMIN.store(is_elevated, Ordering::Relaxed);
        }
    });

    IS_ADMIN.load(Ordering::Relaxed)
}

fn kill_process(process_name: &str) {
    let _ = Command::new("taskkill")
        .args(&["/IM", process_name, "/F"])
        .output();
}
