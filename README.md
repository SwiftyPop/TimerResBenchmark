# TimerResBenchmark

<p align="center"><b>TimerResBenchmark is a tool rewritten in Rust for fine-tuning system timer resolution to achieve precise sleep intervals close to 1 millisecond. This optimization can improve system performance, especially in scenarios like gaming, where consistent frame rates and low latency are critical.</b></p>



---

## Features
- Identifies the most accurate timer resolution for your system.
- Measures sleep intervals to determine the optimal timer resolution.
- No need for PowerShell or Command Prompt—just run the executable.
- Generates results that can be visualized using tools like Plotly Chart Studio.

---

## Installation and Usage

### Step 1: Download and Run the Tool
1. **Disable HPET** and use an **Idle-disabled power plan** (see [Troubleshooting](#troubleshooting) for instructions).
2. Download the latest release from [GitHub Releases](https://github.com/SwiftyPop/TimerResBenchmark/releases).
3. Extract the `.7z` archive.
4. Run `timer_res_benchmark.exe` as an administrator.
   - Adjust settings in `appsettings.json` if needed.
5. The tool will generate a `results.txt` file in the same directory upon completion.

---

### Step 2: Visualize the Results
1. Visit [Plotly Chart Studio](https://chart-studio.plotly.com/create/#/).
2. Click **"Import"** at the top right and upload the `results.txt` file.
3. Add a trace and configure the settings as shown below:
   ![Plotly Configuration](https://github.com/SwiftyPop/TimerResBenchmark/assets/90952326/9f08eb09-7e1a-41f5-819e-10bd41444cd9)
4. Look for the lowest `Sleep(1) Delta` on the y-axis. This represents the most precise and consistent 1ms sleep delays.
   - Example: If the lowest delta is at 0.5024ms, this is your optimal timer resolution.

---

### Step 3: Set the Optimal Timer Resolution
1. Create a shortcut for `SetTimerResolution.exe`.
2. Place the shortcut in the `shell:startup` folder with the following target:
```
C:\PATH\TO\SetTimerResolution.exe --no-console --resolution 5000
```
- Replace `5000` with your optimal resolution (e.g., `5000` for 0.5ms).
3. Restart your PC and verify the settings using `MeasureSleep.exe`.

---

## Troubleshooting

### Sleep Delays Are Spiking (>1ms)
1. **Disable HPET**:
- Open Command Prompt as administrator and run:
  ```bash
  bcdedit /deletevalue useplatformclock
  bcdedit /set disabledynamictick yes
  ```
- On Windows Server 2022+ and Windows 11+, apply this registry change:
  ```plaintext
  [HKEY_LOCAL_MACHINE\SYSTEM\CurrentControlSet\Control\Session Manager\kernel]
  "GlobalTimerResolutionRequests"=dword:00000001
  ```
- Verify HPET status using [TimerBench](https://www.overclockers.at/articles/the-hpet-bug-what-it-is-and-what-it-isnt).

2. **Use an Idle-Disabled Power Plan**:
- Download the [Muren power plan](https://www.mediafire.com/file/39yxlxpbkyjg3qa/Muren.pow/file).
- Import the plan using Command Prompt:
  ```bash
  powercfg -import C:\PATH\TO\MUREN.POW
  ```
- Set your power plan to **Muren**.

---

## Why Use TimerResBenchmark?
- Disables HPET and uses more stable timers (e.g., TSC at 3.32MHz) for better frame rate consistency and lower latency.
- Unlike the original PowerShell-based benchmark, this tool is an executable written in Rust for efficiency and ease of use.
- At first, I rewrite this benchmark tool in C# and now, I rewrite it again Rust as part of learning endeavor :). Contributions and feedback are welcome!

---

## License
This project is licensed under the **MIT License**. See the [LICENSE](https://github.com/SwiftyPop/TimerResBenchmark/blob/master/LICENSE) file for details.
