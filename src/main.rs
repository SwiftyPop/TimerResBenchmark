use std::env;
use std::io::{self, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::Duration;
use std::{fs, mem};
use tokio::task;
use tokio::time::sleep;
use windows_sys::Win32::Foundation::HANDLE;
use windows_sys::Win32::Security::{GetTokenInformation, TokenElevation, TOKEN_ELEVATION, TOKEN_QUERY};
use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

#[derive(serde::Deserialize)]
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
    // Print the current working directory for debugging
    println!("Current directory: {:?}", env::current_dir()?);

    // Check if running as administrator
    if !is_admin() {
        eprintln!("error: administrator privileges required");
        std::process::exit(1);
    }

    // Load configuration from JSON file
    let config = match fs::read_to_string("appsettings.json") {
        Ok(content) => content,
        Err(e) => {
            eprintln!("Failed to read appsettings.json: {}", e);
            std::process::exit(1);
        }
    };
    let parameters: BenchmarkingParameters = match serde_json::from_str(&config) {
        Ok(params) => params,
        Err(e) => {
            eprintln!("Failed to parse JSON: {}", e);
            eprintln!("JSON content: {}", config);
            std::process::exit(1);
        }
    };

    // Calculate estimated time
    let iterations = (parameters.end_value - parameters.start_value) / parameters.increment_value;
    let total_minutes = iterations * parameters.sample_value as f64 * 2.0 / 60_000.0;
    println!(
        "Approximate worst-case estimated time for completion: {:.2} mins",
        total_minutes
    );
    println!("Worst-case is determined by assuming Sleep(1) = ~2ms with 1ms Timer Resolution");
    println!(
        "Start: {}, End: {}, Increment: {}, Samples: {}",
        parameters.start_value, parameters.end_value, parameters.increment_value, parameters.sample_value
    );

    // Get the directory of the current executable
    let exe_dir = env::current_exe()?
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."));

    // Construct absolute paths to the dependencies
    let set_timer_resolution_path = exe_dir.join("SetTimerResolution.exe");
    let measure_sleep_path = exe_dir.join("MeasureSleep.exe");

    // Check if dependencies exist
    let dependencies = vec![&set_timer_resolution_path, &measure_sleep_path];
    let missing_dependencies: Vec<_> = dependencies
        .iter()
        .filter(|dep| !dep.exists())
        .map(|dep| dep.to_str().unwrap_or("").to_string())
        .collect();

    if !missing_dependencies.is_empty() {
        for dep in &missing_dependencies {
            eprintln!("Error: {} does not exist in the executable directory", dep);
        }
        return Ok(());
    }

    // Write header to results file
    fs::write("results.txt", "RequestedResolutionMs,DeltaMs,STDEV\n")?;

    // Benchmark loop
    let mut i = parameters.start_value;
    while i <= parameters.end_value {
        let formatted_value = (i * 10_000.0).round() / 10_000.0;
        println!("info: benchmarking {}", formatted_value);

        let resolution = (formatted_value * 10_000.0) as i32;

        // Debug: Print the command being executed
        println!(
            "Executing: {} --resolution {} --no-console",
            set_timer_resolution_path.display(),
            resolution
        );

        // Clone the path before moving it into the closure
        let set_timer_resolution_path_clone = set_timer_resolution_path.clone();
        task::spawn_blocking(move || {
            Command::new(&set_timer_resolution_path_clone)
                .arg("--resolution")
                .arg(resolution.to_string())
                .arg("--no-console")
                .stdout(Stdio::null())
                .spawn()
                .expect("Failed to start SetTimerResolution.exe");
        })
            .await?;

        // Delay after setting resolution
        sleep(Duration::from_millis(1)).await;

        // Debug: Print the command being executed
        println!(
            "Executing: {} --samples {}",
            measure_sleep_path.display(),
            parameters.sample_value
        );

        let output = Command::new(&measure_sleep_path)
            .arg("--samples")
            .arg(parameters.sample_value.to_string())
            .output()?;

        // Debug: Print the output of the command
        println!("Output: {}", String::from_utf8_lossy(&output.stdout));

        let output_str = String::from_utf8_lossy(&output.stdout);
        let mut avg = 0.0;
        let mut stdev = 0.0;

        for line in output_str.lines() {
            if line.starts_with("Avg: ") {
                avg = line[5..].parse().unwrap_or(0.0);
            } else if line.starts_with("STDEV: ") {
                stdev = line[7..].parse().unwrap_or(0.0);
            }
        }

        let result_line = format!("{:.4}, {:.4}, {:.4}\n", formatted_value, avg, stdev);
        fs::OpenOptions::new()
            .append(true)
            .open("results.txt")?
            .write_all(result_line.as_bytes())?;

        kill_process("SetTimerResolution.exe");

        // Increment by the specified value
        i += parameters.increment_value;
    }

    println!("info: results saved in results.txt");
    Ok(())
}

fn is_admin() -> bool {
    unsafe {
        let mut token: HANDLE = std::ptr::null_mut();
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
    let output = Command::new("taskkill")
        .arg("/IM")
        .arg(process_name)
        .arg("/F")
        .output();

    if let Err(e) = output {
        eprintln!("Failed to kill process {}: {}", process_name, e);
    }
}