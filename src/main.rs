use io::ErrorKind;
use serde::{Deserialize, Serialize};
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
use comfy_table::{Cell, Color, ContentArrangement, Row, Table};
use colored::Colorize;

#[derive(Debug, Deserialize, Serialize)]
struct BenchmarkingParameters {
    #[serde(rename = "StartValue", deserialize_with = "validate_positive_f64")]
    start_value: f64,
    #[serde(rename = "IncrementValue", deserialize_with = "validate_positive_f64")]
    increment_value: f64,
    #[serde(rename = "EndValue", deserialize_with = "validate_positive_f64")]
    end_value: f64,
    #[serde(rename = "SampleValue", deserialize_with = "validate_positive_i32")]
    sample_value: i32,
}

fn validate_positive_f64<'de, D>(deserializer: D) -> Result<f64, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = f64::deserialize(deserializer)?;
    if value > 0.0 {
        Ok(value)
    } else {
        Err(serde::de::Error::custom("Value must be positive"))
    }
}

fn validate_positive_i32<'de, D>(deserializer: D) -> Result<i32, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = i32::deserialize(deserializer)?;
    if value > 0 {
        Ok(value)
    } else {
        Err(serde::de::Error::custom("Value must be positive"))
    }
}


#[tokio::main]
async fn main() -> io::Result<()> {
    use colored::*;

    // Create a dynamic separator using '=' characters
    let separator = "=".repeat(60);
    
    // Title Block
    println!("\n{}", separator);
    println!("{:^60}", "🚀 Timer Resolution Benchmark Tool".bold().cyan());
    println!("{}\n", separator);

    // Check admin privileges first - fail fast
    if !is_admin() {
        eprintln!("{} {}", "❌ Error:".bold().red(), "Administrator privileges required!".bold().red());
        eprintln!("   {}", "Please run this program as Administrator.".bold().red());
        return Err(Error::new(ErrorKind::PermissionDenied, "Administrator privileges required"));
    }

    // System information block
    println!("{}", "📊 System Information".bold().yellow());
    println!("━━━━━━━━━━━━━━━━━━━");
    println!("📂 Working Directory: {}", env::current_dir()?.display());
    println!("🛡️ Admin Privileges: {}", "✓ Confirmed".bold().green());

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
    println!("{}", "🔧 System Configuration".bold().yellow());
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
            let mut prompt = |desc: &str, current: &str| -> io::Result<Option<String>> {
                println!("▸ {}: {} (current)", desc, current);
                println!("Enter new {} (or press Enter to keep current): ", desc);
                input.clear();
                io::stdin().read_line(&mut input)?;
                let trimmed = input.trim();
                Ok(if trimmed.is_empty() { None } else { Some(trimmed.to_string()) })
            };

            println!("⚙️ Benchmark Parameters");
            println!("━━━━━━━━━━━━━━━━━━━");

            if let Some(new_value) = prompt("Start Value", &format!("{:.4} ms", params.start_value))? {
                params.start_value = new_value.parse().map_err(|e| Error::new(ErrorKind::InvalidInput, e))?;
            }
            if let Some(new_value) = prompt("Increment Value", &format!("{:.4} ms", params.increment_value))? {
                params.increment_value = new_value.parse().map_err(|e| Error::new(ErrorKind::InvalidInput, e))?;
            }
            if let Some(new_value) = prompt("End Value", &format!("{:.4} ms", params.end_value))? {
                params.end_value = new_value.parse().map_err(|e| Error::new(ErrorKind::InvalidInput, e))?;
            }
            if let Some(new_value) = prompt("Sample Value", &params.sample_value.to_string())? {
                params.sample_value = new_value.parse().map_err(|e| Error::new(ErrorKind::InvalidInput, e))?;
            }

            let iterations = ((params.end_value - params.start_value) / params.increment_value).ceil();
            println!("▸ Iterations:   {}\n", iterations as i32);

            // Save updated parameters back to appsettings.json
            if let Err(e) = fs::write("appsettings.json", serde_json::to_string_pretty(&params)?) {
                eprintln!("❌ Failed to save updated parameters: {}", e);
            }

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
            .template(
                "{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {pos}/{len} \n\
                ▸ Current Resolution: {msg:.yellow} \n\
                ▸ ETA: {eta_precise} | Iteration Time: {per_sec}"
            )
            .map_err(|e| Error::new(ErrorKind::Other, e.to_string()))?
            .progress_chars("█▓▒░ ")
            .with_key("per_sec", |state: &indicatif::ProgressState, w: &mut dyn std::fmt::Write| write!(w, "{:.2}s/iter", state.per_sec()).unwrap())
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

    let results_content = fs::read_to_string(&results_path)?;
    let mut rdr = csv::ReaderBuilder::new()
        .has_headers(true)
        .from_reader(results_content.as_bytes());

    let mut all_results = Vec::new();
    let mut optimal_resolution = None;
    let mut min_delta = f64::MAX;
    let mut min_std = f64::MAX;

    for result in rdr.records().filter_map(|r| r.ok()) {
        if let (Ok(res), Ok(delta), Ok(std)) = (
            result[0].parse::<f64>(),
            result[1].parse::<f64>(),
            result[2].parse::<f64>(),
        ) {
            all_results.push((res, delta, std));

            // Find the optimal resolution
            if delta < min_delta || (delta == min_delta && std < min_std) {
                optimal_resolution = Some(res);
                min_delta = delta;
                min_std = std;
            }
        }
    }
    // Call print_summary if valid results are found
if let Some(resolution) = optimal_resolution {
    print_summary(resolution, &all_results);
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
fn print_summary(optimal_res: f64, all_results: &[(f64, f64, f64)]) {
    assert!(!all_results.is_empty(), "Cannot print summary of empty results");

    let mut table = Table::new();
    table.set_content_arrangement(ContentArrangement::Dynamic);
    table.set_header(vec![
        Cell::new("Resolution (ms)"),
        Cell::new("Avg Δ (ms)"),
        Cell::new("STDEV"),
    ]);

    let mut optimal_row_index = None;

    for (i, &(res, avg, stdev)) in all_results.iter().enumerate() {
        // Check if this row is optimal based on a tolerance
        let is_optimal = (res - optimal_res).abs() < f64::EPSILON;
        if is_optimal {
            optimal_row_index = Some(i);
        }

        // Create cells with formatted numbers.
        // We use comfy_table's Cell and its styling API, so the styling does not affect width calculations.
        let res_cell = if is_optimal {
            Cell::new(format!("{:>8.4}", res)).fg(Color::Green)
        } else {
            Cell::new(format!("{:>8.4}", res))
        };
        let avg_cell = if is_optimal {
            Cell::new(format!("{:>8.4}", avg)).fg(Color::Green)
        } else {
            Cell::new(format!("{:>8.4}", avg))
        };
        let stdev_cell = if is_optimal {
            Cell::new(format!("{:>8.4}", stdev)).fg(Color::Green)
        } else {
            Cell::new(format!("{:>8.4}", stdev))
        };

        let row = Row::from(vec![res_cell, avg_cell, stdev_cell]);
        table.add_row(row);
    }

    assert!(optimal_row_index.is_some(), "Optimal resolution not found in results");

    // Finally, print the table.
    println!("{}", table);

    // Print the header and optimal resolution.
    println!("\n{}", "OPTIMAL RESOLUTION FOUND".green().bold());
    println!("{}", "═════════════════════════");
    println!("▸ {:>8.4} ms\n", optimal_res);
}
    

lazy_static::lazy_static! {
    static ref HPET_STATUS: Mutex<Option<String>> = Mutex::new(None);
}

fn check_hpet_status() -> io::Result<()> {
    let mut status = HPET_STATUS.lock().unwrap();

    // Use the cached status if available.
    if let Some(ref cached_status) = *status {
        println!("HPET status (cached): {}", cached_status);
        return Ok(());
    }

    // Run the bcdedit command to get the current boot configuration.
    let output = Command::new("bcdedit")
        .arg("/enum")
        .arg("{current}")
        .output()?;

    if !output.status.success() {
        eprintln!("❌ Error: Failed to retrieve HPET status");
        return Err(Error::new(ErrorKind::Other, "Failed to retrieve HPET status"));
    }

    let output_str = String::from_utf8_lossy(&output.stdout);

    // We'll capture the values for the two keys if they exist.
    let mut useplatformclock_value: Option<String> = None;
    let mut disabledynamictick_value: Option<String> = None;

    for line in output_str.lines() {
        let mut parts = line.split_whitespace();
        if let (Some(key), Some(value)) = (parts.next(), parts.next()) {
            match key.to_lowercase().as_str() {
                "useplatformclock" => {
                    useplatformclock_value = Some(value.to_lowercase());
                }
                "disabledynamictick" => {
                    disabledynamictick_value = Some(value.to_lowercase());
                }
                _ => {}
            }
        }
    }

    // Decide HPET status.
    // According to the requirement, if "useplatformclock" is absent and "disabledynamictick" is "yes",
    // then we consider HPET as disabled.
    let hpet_status = match (
        useplatformclock_value.as_deref(),
        disabledynamictick_value.as_deref(),
    ) {
        // If "useplatformclock" is present and equals "no", and disabledynamictick is "yes" → disabled.
        (Some("no"), Some("yes")) => "disabled",
        // If "useplatformclock" is absent but disabledynamictick is "yes" → disabled.
        (None, Some("yes")) => "disabled",
        // If both keys are absent, default to disabled.
        (None, None) => "disabled",
        // In all other cases, consider HPET as enabled.
        _ => "enabled",
    };

    println!("HPET status: {}", hpet_status);

    // If HPET is enabled, notify the user and prompt to disable.
    if hpet_status == "enabled" {
        println!("⚠️ HPET is enabled. For optimal results, it is recommended to disable HPET.");
        println!("Please refer to the troubleshooting guide: https://github.com/SwiftyPop/TimerResBenchmark?tab=readme-ov-file#troubleshooting");
        println!("Would you like to disable HPET now? (y/n): ");
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        if input.trim().eq_ignore_ascii_case("y") {
            if let Err(e) = disable_hpet() {
                eprintln!("❌ Error: Failed to disable HPET: {}", e);
                return Err(e);
            }
            println!("✅ HPET has been disabled. Please restart your computer for the changes to take effect.");
        }
    }

    *status = Some(hpet_status.to_string());

    Ok(())
}

fn disable_hpet() -> io::Result<()> {
    let mut commands = vec![
        {
            let mut cmd = Command::new("bcdedit");
            cmd.arg("/deletevalue").arg("useplatformclock");
            cmd
        },
        {
            let mut cmd = Command::new("bcdedit");
            cmd.arg("/set").arg("disabledynamictick").arg("yes");
            cmd
        },
    ];

    if let Err(e) = apply_registry_tweak() {
        eprintln!("❌ Error: Failed to apply registry tweak: {}", e);
        return Err(e.into());
    }

    for command in commands.iter_mut() {
        let output = command.output()
            .map_err(|e| io::Error::new(io::ErrorKind::Other, format!("Failed to disable HPET: {}", e)))?;
        if !output.status.success() {
            return Err(io::Error::new(
                io::ErrorKind::Other,
                format!("Failed to disable HPET: {}", output.status),
            ));
        }
    }

    Ok(())
}

fn apply_registry_tweak() -> io::Result<()> {
    let output = Command::new("reg")
        .arg("add")
        .arg(r"HKEY_LOCAL_MACHINE\SYSTEM\CurrentControlSet\Control\Session Manager\kernel")
        .arg("/v")
        .arg("GlobalTimerResolutionRequests")
        .arg("/t")
        .arg("REG_DWORD")
        .arg("/d")
        .arg("1")
        .arg("/f")
        .output()?;

    if !output.status.success() {
        return Err(Error::new(
            ErrorKind::Other,
            "Failed to apply registry tweak",
        ));
    }

    Ok(())
}

fn parse_measurement_output(output: &[u8]) -> io::Result<(f64, f64)> {
    let output_str = std::str::from_utf8(output)
        .map_err(|e| Error::new(ErrorKind::InvalidData, e))?;

    let mut avg = None;
    let mut stdev = None;

    for line in output_str.lines() {
        if avg.is_none() && line.starts_with("Avg: ") {
            avg = line[5..].parse().ok();
        } else if stdev.is_none() && line.starts_with("STDEV: ") {
            stdev = line[7..].parse().ok();
        }

        if avg.is_some() && stdev.is_some() {
            break;
        }
    }

    avg.zip(stdev).ok_or_else(|| Error::new(ErrorKind::InvalidData, "Invalid measurement output"))
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

    for (_pid, process) in system.processes() {
        if process.name().eq_ignore_ascii_case(process_name) {
            if process.kill() {
                found = true;
            } else {
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


