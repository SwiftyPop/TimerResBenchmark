use std::borrow::Cow;
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
use os_info;
use raw_cpuid;
use windows_sys::Win32::Foundation::HANDLE;
use windows_sys::Win32::Security::{GetTokenInformation, TokenElevation, TOKEN_ELEVATION, TOKEN_QUERY};
use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Once;
use indicatif;
use std::sync::Mutex;

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
    println!("🛡️ Admin Privileges: ✓ Confirmed");

    // Display current executable path
    let exe_path = env::current_exe()?;
    println!("🔍 Executable Path: {}", exe_path.display());

    // Display OS information
    let os_info = os_info::get();

    // Check if the OS is Windows and display specific version information
    if let os_info::Type::Windows = os_info.os_type() {
        if let Some(build_number) = os_info.version().to_string().split('.').nth(2).and_then(|s| s.parse::<u32>().ok()) {
            let version = if build_number >= 22000 {
                "Windows 11"
            } else {
                "Windows 10"
            };
            println!("🖥️ Windows Version: {} (Build {})", version, build_number);
        } else {
            println!("🖥️ Windows Version: Unknown Build");
        }
    }

    // Display CPU information
    let cpuid = raw_cpuid::CpuId::new();

    // Get the CPU brand string
    if let Some(brand) = cpuid.get_processor_brand_string() {
        println!("💻 CPU: {}", brand.as_str().trim());
    } else {
        println!("💻 CPU: Unknown");
    }

    println!();

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
            let mut input = Cow::Borrowed("");

            let mut prompt = |desc: &str, current: &str| -> io::Result<Cow<'static, str>> {
                println!("▸ {}: {} (current)", desc, current);
                println!("Enter new {} (or press Enter to keep current): ", desc);
                input.to_mut().clear();
                io::stdin().read_line(input.to_mut())?;
                Ok(Cow::Owned(input.trim().to_string()))
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

    let exe_dir = env::current_exe()?.parent()
        .ok_or_else(|| {
            eprintln!("❌ Error: Failed to get current executable path");
            Error::new(ErrorKind::Other, "Failed to get current executable path")
        })?
        .to_path_buf();

    let set_timer_resolution_path = exe_dir.join("SetTimerResolution.exe");
    let measure_sleep_path = exe_dir.join("MeasureSleep.exe");

    // Dependency check
    println!("\n🔍 Checking Dependencies");
    println!("━━━━━━━━━━━━━━━━━━━━━");

    let dependencies = [
        ("SetTimerResolution.exe", &set_timer_resolution_path),
        ("MeasureSleep.exe", &measure_sleep_path),
    ];

    let missing_dependencies: Vec<_> = dependencies.iter()
        .filter_map(|(name, path)| {
            if path.exists() {
                println!("✓ Found: {}", path.file_name().unwrap_or_default().to_string_lossy());
                None
            } else {
                Some(*name)
            }
        })
        .collect();

    if !missing_dependencies.is_empty() {
        eprintln!("❌ Error: Missing dependencies: {}", missing_dependencies.join(", "));
        return Err(Error::new(ErrorKind::NotFound, "Missing dependencies"));
    }
    println!();

    prompt_user("⏳ Press Enter to start the benchmark...")?;
    
    fn prompt_user(message: &str) -> io::Result<()> {
        println!("{}", message);
        io::stdin().read_line(&mut String::new())?;
        Ok(())
    }

    let results_file = fs::File::create("results.txt")?;
    let mut results_writer = BufWriter::with_capacity(8 * 1024, results_file);
    writeln!(results_writer, "RequestedResolutionMs,DeltaMs,StandardDeviation")?;
    println!("📝 Results will be saved to: results.txt");

    let mut current = parameters.start_value;
    let total_iterations = ((parameters.end_value - parameters.start_value) / parameters.increment_value).ceil() as u64;
    let progress_bar = indicatif::ProgressBar::new(total_iterations);
    progress_bar.set_style(
        indicatif::ProgressStyle::default_bar()
            .template("[{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({eta}) {msg}")
            .unwrap()
            .progress_chars("#>-")
    );
    progress_bar.enable_steady_tick(Duration::from_millis(100));

    while current <= parameters.end_value {
        let resolution = ((current * 10_000.0).round() / 10_000.0 * 10_000.0) as i32;

        let set_timer_resolution_path = set_timer_resolution_path.clone();
        let set_timer_result = task::spawn_blocking({
            move || {
            Command::new(&set_timer_resolution_path)
                .args(&["--resolution", &resolution.to_string(), "--no-console"])
                .stdout(Stdio::null())
                .spawn()
            }
        }).await?;

        if let Err(e) = set_timer_result {
            eprintln!("Failed to set timer resolution: {}", e);
        }

        sleep(Duration::from_millis(1)).await;

        let output = Command::new(&measure_sleep_path)
            .arg("--samples")
            .arg(parameters.sample_value.to_string())
            .output()
            .map_err(|e| {
                eprintln!("Failed to execute measurement: {}", e);
                e
            })?;

        let (avg, stdev) = parse_measurement_output(&output.stdout).map_err(|e| {
            eprintln!("Failed to parse measurement output: {}", e);
            e
        })?;

        if avg != 0.0 && stdev != 0.0 {
            writeln!(results_writer, "{:.4},{:.4},{:.4}", current, avg, stdev)?;
        } else {
            eprintln!("❌ Measurement output is invalid for resolution: {}", resolution);
        }

        results_writer.flush()?;
        if let Err(e) = kill_process("SetTimerResolution.exe") {
            eprintln!("Failed to kill process: {}", e);
        }

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

    let (optimal_resolution, _, _) = rdr.records()
        .filter_map(|result| {
            result.ok().and_then(|record| {
                let requested_resolution: f64 = record[0].parse().ok()?;
                let delta_ms: f64 = record[1].parse().ok()?;
                let std_dev: f64 = record[2].parse().ok()?;
                Some((requested_resolution, delta_ms, std_dev))
            })
        })
        .fold((None, f64::MAX, f64::MAX), |(opt_res, min_delta, min_std), (res, delta, std)| {
            if delta < min_delta || (delta == min_delta && std < min_std) {
                (Some(res), delta, std)
            } else {
                (opt_res, min_delta, min_std)
            }
        });

    if let Some(resolution) = optimal_resolution {
        println!("✅ Optimal Timer Resolution: {:.4} ms", resolution);
    } else {
        eprintln!("❌ Error: No valid data found in results.txt");
        return Err(Error::new(ErrorKind::InvalidData, "No valid data found in results.txt"));
    }

    println!("Benchmarking completed successfully");

    // Wait for user input before exiting
    prompt_exit()?;

    Ok(())
}

fn prompt_exit() -> io::Result<()> {
    println!("Press Enter to exit...");
    io::stdin().read_line(&mut String::new())?;
    Ok(())
}
    

lazy_static::lazy_static! {
    static ref HPET_STATUS: Mutex<Option<String>> = Mutex::new(None);
}

fn check_hpet_status() -> io::Result<()> {
    let mut status = HPET_STATUS.lock().unwrap();

    if let Some(ref cached_status) = *status {
        println!("HPET status (cached): {}", cached_status);
        return Ok(());
    }

    let output = Command::new("bcdedit")
        .args(&["/enum", "{current}"])
        .output()?;

    if !output.status.success() {
        eprintln!("❌ Error: Failed to retrieve HPET status");
        return Err(Error::new(ErrorKind::Other, "Failed to retrieve HPET status"));
    }

    let output_str = String::from_utf8_lossy(&output.stdout);
    let hpet_status = output_str.lines()
        .filter_map(|line| {
            let mut parts = line.split_whitespace();
            match (parts.next(), parts.next()) {
                (Some("useplatformtick"), Some(value)) => Some(("useplatformtick", value.to_lowercase())),
                (Some("disabledynamictick"), Some(value)) => Some(("disabledynamictick", value.to_lowercase())),
                _ => None,
            }
        })
        .collect::<std::collections::HashMap<_, _>>();

    let hpet_status = match (hpet_status.get("useplatformtick").map(String::as_str), hpet_status.get("disabledynamictick").map(String::as_str)) {
        (Some("no"), Some("yes")) | (None, Some("yes")) | (None, None) => "disabled",
        _ => "enabled",
    };

    println!("HPET status: {}", hpet_status);

    if hpet_status == "enabled" {
        println!("⚠️ HPET is enabled. For optimal results, it is recommended to disable HPET.");
        println!("Please refer to the troubleshooting guide: https://github.com/SwiftyPop/TimerResBenchmark?tab=readme-ov-file#troubleshooting");
    }

    *status = Some(hpet_status.to_string());

    Ok(())
}

fn parse_measurement_output(output: &[u8]) -> io::Result<(f64, f64)> {
    let output_str = std::str::from_utf8(output)
        .map_err(|e| Error::new(ErrorKind::InvalidData, e))?;

    let mut avg = None;
    let mut stdev = None;

    for line in output_str.lines() {
        if let Some(value) = line.strip_prefix("Avg: ") {
            avg = value.parse().ok();
        } else if let Some(value) = line.strip_prefix("STDEV: ") {
            stdev = value.parse().ok();
        }
    }

    match (avg, stdev) {
        (Some(avg), Some(stdev)) => Ok((avg, stdev)),
        _ => Err(Error::new(ErrorKind::InvalidData, "Invalid measurement output")),
    }
}

static IS_ADMIN: AtomicBool = AtomicBool::new(false);
static INIT: Once = Once::new();

fn is_admin() -> bool {
    INIT.call_once(|| {
        unsafe {
            let mut token: HANDLE = ptr::null_mut();
            if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) != 0 {
                let mut elevation: TOKEN_ELEVATION = mem::zeroed();
                let mut size = size_of::<TOKEN_ELEVATION>() as u32;

                if GetTokenInformation(
                    token,
                    TokenElevation,
                    &mut elevation as *mut _ as *mut std::ffi::c_void,
                    size,
                    &mut size,
                ) != 0 && elevation.TokenIsElevated != 0
                {
                    IS_ADMIN.store(true, Ordering::Relaxed);
                }
                windows_sys::Win32::Foundation::CloseHandle(token);
            }
        }
    });

    IS_ADMIN.load(Ordering::Relaxed)
}

fn kill_process(process_name: &str) -> io::Result<()> {
    let mut system = sysinfo::System::new_all();
    system.refresh_all();

    let mut found = false;

    for (pid, process) in system.processes() {
        if process.name().eq_ignore_ascii_case(process_name) {
            if process.kill() {
                println!("Killed process {} with PID {}", process_name, pid);
                found = true;
            } else {
                eprintln!("Failed to kill process {} with PID {}", process_name, pid);
                return Err(Error::new(ErrorKind::Other, format!("Failed to kill process {}", process_name)));
            }
        }
    }

    if !found {
        eprintln!("Process {} not found", process_name);
        return Err(Error::new(ErrorKind::NotFound, format!("Process {} not found", process_name)));
    }

    Ok(())
}


