use io::ErrorKind;
use serde::Deserialize;
use std::io::{self, BufWriter, Error, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::Duration;
use std::{env, ptr};
use std::{fs, mem};
use tokio::task;
use tokio::time::sleep;
//use windows::core::imp::GetProcAddress;
use windows_sys::Win32::Foundation::HANDLE;
use windows_sys::Win32::Security::{GetTokenInformation, TokenElevation, TOKEN_ELEVATION, TOKEN_QUERY};
use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

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
    // Enhanced logging and error handling
    println!("Starting benchmarking script...");
    println!("Current directory: {:?}", env::current_dir()?);

    if !is_admin() {
        eprintln!("Error: Administrator privileges required");
        return Err(Error::new(ErrorKind::PermissionDenied, "Administrator privileges required"));
    }

    // New system checks
    println!("\nChecking system configuration...");
    if let Err(e) = check_hpet_status() {
        eprintln!("Warning: Could not determine HPET status: {}", e);
    }

   

    // Detailed configuration loading
    let config_path = "appsettings.json";
    println!("Attempting to read configuration from: {}", config_path);

    let config = match fs::read_to_string(config_path) {
        Ok(content) => content,
        Err(e) => {
            eprintln!("Failed to read configuration file: {}", e);
            return Err(e);
        }
    };

    println!("Configuration content: {}", config);

    let parameters: BenchmarkingParameters = match serde_json::from_str(&config) {
        Ok(params) => {
            println!("Configuration parsed successfully: {:?}", params);
            params
        },
        Err(e) => {
            eprintln!("Failed to parse configuration JSON: {}", e);
            return Err(Error::new(ErrorKind::InvalidData, e));
        }
    };

    let iterations = (parameters.end_value - parameters.start_value) / parameters.increment_value;
    let total_minutes = iterations * parameters.sample_value as f64 * 2.0 / 60_000.0;
    println!(
        "Estimated completion time: {:.2} mins (worst-case with 1ms Timer Resolution)",
        total_minutes
    );

    let exe_dir = env::current_exe()?
        .parent()
        .map_or_else(|| PathBuf::from("."), |p| p.to_path_buf());

    let set_timer_resolution_path = exe_dir.join("SetTimerResolution.exe");
    let measure_sleep_path = exe_dir.join("MeasureSleep.exe");

    println!("Timer resolution executable: {}", set_timer_resolution_path.display());
    println!("Measure sleep executable: {}", measure_sleep_path.display());

    for dep in &[&set_timer_resolution_path, &measure_sleep_path] {
        if !dep.exists() {
            eprintln!("Dependency not found: {}", dep.display());
            return Err(Error::new(ErrorKind::NotFound,
                                      format!("Dependency not found: {}", dep.display())));
        }
    }

    let mut results_file = BufWriter::new(fs::File::create("results.txt")?);
    writeln!(results_file, "RequestedResolutionMs,DeltaMs,STDEV")?;

    let mut current = parameters.start_value;
    while current <= parameters.end_value {
        let resolution = ((current * 10_000.0).round() / 10_000.0 * 10_000.0) as i32;

        println!("Processing resolution: {}", resolution);

        let timer_path = set_timer_resolution_path.clone();
        let sleep_path = measure_sleep_path.clone();

        task::spawn_blocking(move || {
            println!("Setting timer resolution: {}", resolution);
            match Command::new(&timer_path)
                .args(&["--resolution", &resolution.to_string(), "--no-console"])
                .stdout(Stdio::null())
                .spawn() {
                Ok(_) => println!("Timer resolution set successfully"),
                Err(e) => eprintln!("Failed to set timer resolution: {}", e)
            }
        }).await?;

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

        writeln!(results_file, "{:.4},{:.4},{:.4}", current, avg, stdev)?;

        kill_process("SetTimerResolution.exe");

        current += parameters.increment_value;
    }

    println!("Benchmarking completed successfully");
    Ok(())
}

fn check_hpet_status() -> io::Result<()> {
    let output = Command::new("bcdedit")
        .args(&["/enum", "{current}"])
        .output()?;

    if !output.status.success() {
        let err_msg = String::from_utf8_lossy(&output.stderr);
        return Err(Error::new(ErrorKind::Other, format!("bcdedit failed: {}", err_msg)));
    }

    let output_str = String::from_utf8_lossy(&output.stdout);
    let mut useplatformtick = "no".to_string();
    let mut disabledynamictick = "no".to_string();

    for line in output_str.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 2 {
            let key = parts[0].to_lowercase();
            let value = parts[1].to_lowercase();

            match key.as_str() {
                "useplatformtick" => {
                    useplatformtick = value;
                }
                "disabledynamictick" => {
                    disabledynamictick = value;
                }
                _ => {}
            }
        }
    }

    let hpet_status = if useplatformtick == "no" && disabledynamictick == "yes" {
        "disabled"
    } else {
        "enabled"
    };

    println!("HPET status: {}", hpet_status);
    Ok(())
}


// Rest of the code remains the same as in previous version
fn parse_measurement_output(output: &[u8]) -> io::Result<(f64, f64)> {
    let output_str = std::str::from_utf8(output)
        .map_err(|e| Error::new(ErrorKind::InvalidData, e))?;

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
}

// Existing admin and process kill functions remain the same
fn is_admin() -> bool {
    unsafe {
        let mut token: HANDLE = ptr::null_mut();
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) == 0 {
            return false;
        }

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
    }
}

fn kill_process(process_name: &str) {
    let _ = Command::new("taskkill")
        .args(&["/IM", process_name, "/F"])
        .output();
}